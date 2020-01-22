use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::convert::Infallible;
use std::net::SocketAddr;

use std::io;
use std::io::{Error, ErrorKind};

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

async fn get_metrics(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let cpu_metrics = collect_gz_cpu_util_metrics()
        .await
        .expect("failed to read kstats");
    Ok(Response::new(cpu_metrics.into()))
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
