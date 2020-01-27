use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::convert::Infallible;
use std::net::SocketAddr;

use std::io;
use std::io::{Error, ErrorKind};
use std::num;

struct CpuKstatMetric {
    kstat_key: String,
    name: String,
    mtype: String,
    desc: String,
}

async fn collect_gz_cpu_util_metrics() -> io::Result<String> {
    // List of cpu kstats we export.
    let cpu_util_kstat_metrics = vec![
        CpuKstatMetric {
            kstat_key: "cpu_nsec_idle".into(),
            name: "cpu_idle_seconds_total".into(),
            mtype: "counter".into(),
            desc: "CPU idle time in seconds".into(),
        },
        CpuKstatMetric {
            kstat_key: "cpu_nsec_kernel".into(),
            name: "cpu_kernel_seconds_total".into(),
            mtype: "counter".into(),
            desc: "CPU kernel time in seconds".into(),
        },
        CpuKstatMetric {
            kstat_key: "cpu_nsec_user".into(),
            name: "cpu_user_seconds_total".into(),
            mtype: "counter".into(),
            desc: "CPU user time in seconds".into(),
        },
        CpuKstatMetric {
            kstat_key: "cpu_nsec_dtrace".into(),
            name: "cpu_dtrace_seconds_total".into(),
            mtype: "counter".into(),
            desc: "CPU dtrace time in seconds".into(),
        },
    ];

    /*
     * Pull cpu kstats. This vector contains kstat info
     * for all the cpus in the system.
     */
    let reader = kstat::KstatReader::new(Some("cpu"), None, Some("sys"), None)?;
    let cpu_stats = reader.read()?;

    // Metrics string to be returned
    let mut metrics_string = String::from("");

    // Iterate over the stats to be exported
    for metric in cpu_util_kstat_metrics {
        // Generate metric header
        let metric_header = format!(
            "# HELP {} {}\n# TYPE {} {}\n",
            &metric.name, &metric.desc, &metric.name, &metric.mtype
        );

        /*
         * For every metric we loop through cpu kstats. This way we generate
         * a metric for every cpu. Idividual metrics are labeled by cpu_id
         */
        let mut metric_values = String::from("");
        for stat in &cpu_stats {
            // Extract stat named value
            let value = stat.data.get(&metric.kstat_key).ok_or_else(|| {
                Error::new(
                    ErrorKind::NotFound,
                    format!("kstat metric not found: {}", metric.kstat_key),
                )
            })?;

            let value_nsec = match value {
                kstat::kstat_named::KstatNamedData::DataUInt64(v) => Ok(v),
                _ => Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "Unexpected kstat {} type, not DataUInt64",
                        &metric.kstat_key
                    ),
                )),
            }?;

            // Prometheus expects metric values to be float64
            let value_sec = *value_nsec as f64 / 10e9;

            // Metric body
            let body = format!(
                "{}{{cpu_id=\"{}\"}} {}\n",
                &metric.name, stat.instance, value_sec
            );
            metric_values.push_str(&body);
        }

        metrics_string.push_str(&metric_header);
        metrics_string.push_str(&metric_values);
    }

    Ok(metrics_string)
}

struct ZpoolListMetric {
    key: String,
    units: String,
    desc: String,
    index: usize,
    encoder: Box<dyn Fn(String) -> Result<f64, num::ParseFloatError> + Send + Sync>,
}

use tokio::process::Command;

async fn get_zpool_metrics() -> io::Result<String> {
    let zpool_list_metrics = vec![
        ZpoolListMetric {
            key: "allocated".into(),
            units: "bytes".into(),
            desc: "Amount of storage space used withing the pool".into(),
            index: 1,
            encoder: Box::new(|x| x.parse()),
        },
        ZpoolListMetric {
            key: "fragmentation".into(),
            units: "percent".into(),
            desc: "Amount of fragmentation in the pool".into(),
            index: 2,
            //chop off the trailing '%' if present.
            encoder: Box::new(|x| x.replace("%", "").parse()),
        },
        ZpoolListMetric {
            key: "health".into(),
            units: "status".into(),
            desc: "The current health of the pool (\
                   0 = ONLINE, 1 = DEGRADED, 2 = FAULTED, \
                   3 = OFFLINE, 4 = REMOVED, 5 = UNAVAIL, \
                   -1 = UNKNOWN)"
                .into(),
            index: 3,
            encoder: Box::new(|x| {
                Ok(match x.as_str() {
                    "ONLINE" => 0.0,
                    "DEGRADED" => 1.0,
                    "FAULTED" => 2.0,
                    "OFFLINE" => 3.0,
                    "REMOVED" => 4.0,
                    "UNAVAIL" => 5.0,
                    _ => -1.0,
                })
            }),
        },
        ZpoolListMetric {
            key: "size".into(),
            units: "bytes".into(),
            desc: "Zpool size in bytes".into(),
            index: 4,
            encoder: Box::new(|x| x.parse()),
        },
    ];

    // Setting kill_on_drop to kill zpool process in case we hit a timeout
    let zpool_process = Command::new("zpool")
        .args(&["list", "-Hpo", "name,allocated,fragmentation,health,size"])
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let outputs_fut = zpool_process.wait_with_output();

    use tokio::time;
    // XXX: Timeout is set to 5 seconds. This needs to be changed.
    let timeout = time::timeout(time::Duration::from_millis(5000), outputs_fut);
    let outputs_res = timeout.await?; // Handle timeout
    let outputs = outputs_res?; // Fetched outputs successfully

    if !outputs.status.success() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "zpool list  exit code is not 0",
        ));
    }

    let stdout: String = String::from_utf8_lossy(&outputs.stdout).into();

    let mut metrics = "".to_string();

    for metric in zpool_list_metrics {
        let metric_name = format!("zpool_{}_{}", metric.key, metric.units);
        let metric_header = format!(
            "# HELP {} {}\n# TYPE {} gauge\n",
            metric_name, metric.desc, metric_name
        );

        let mut metric_values = "".to_string();

        for line in stdout.lines() {
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() != 5 {
                return Err(Error::new(ErrorKind::InvalidData, "Invalid value......"));
            }

            let value = (metric.encoder)(fields[metric.index].to_string())
                .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid value......"))?;
            metric_values += &format!("{}{{pool=\"{}\"}} {}\n", metric_name, fields[0], value);
        }

        metrics.push_str(&metric_header);
        metrics.push_str(&metric_values);
    }

    println!("{}", metrics);

    Ok(metrics)
}

async fn get_metrics(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let cpu_metrics = collect_gz_cpu_util_metrics()
        .await
        .expect("failed to read kstats");

    let zpool_metrics = get_zpool_metrics()
        .await
        .expect("failed to get zpool metrics");

    let metrics = cpu_metrics + &zpool_metrics;
    Ok(Response::new(metrics.into()))
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    // A `Service` is needed for every connection, so this
    // creates one from our `get_metrics()` function.
    let make_svc = make_service_fn(|_conn| {
        async {
            // service_fn converts our function into a `Service`
            Ok::<_, Infallible>(service_fn(get_metrics))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    // Run this server for... forever!
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
