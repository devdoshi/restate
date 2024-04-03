// Copyright (c) 2024 -  Restate Software, Inc., Restate GmbH.
// All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::path::PathBuf;
use std::str::FromStr;

use enumset::EnumSet;
use humantime::Duration;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::net::{AdvertisedAddress, BindAddress};
use crate::nodes_config::Role;
use crate::PlainNodeId;

use super::{AwsOptions, HttpOptions, RocksDbOptions};

const DEFAULT_STORAGE_DIRECTORY: &str = "restate-data";

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, derive_builder::Builder)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schemars", schemars(default))]
#[serde(rename_all = "kebab-case")]
#[builder(default)]
pub struct CommonOptions {
    /// Defines the roles which this Restate node should run, by default the node
    /// starts with all roles.
    #[cfg_attr(feature = "schemars", schemars(with = "Vec<String>"))]
    pub roles: EnumSet<Role>,

    /// # Node Name
    ///
    /// Unique name for this node in the cluster. The node must not change unless
    /// it's started with empty local store. It defaults to the node hostname.
    pub node_name: String,

    /// If set, the node insists on acquiring this node ID.
    pub force_node_id: Option<PlainNodeId>,

    /// # Cluster Name
    ///
    /// A unique identifier for the cluster. All nodes in the same cluster should
    /// have the same.
    pub cluster_name: String,

    /// If true, then a new cluster is bootstrapped. This node *must* be has an admin
    /// role and a new nodes configuration will be created that includes this node.
    pub allow_bootstrap: bool,

    /// The working directory which this Restate node should use for relative paths. The default is
    /// `restate-data` under the current working directory.
    pub base_dir: PathBuf,

    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    /// Address of the metadata store server to bootstrap the node from.
    pub metadata_store_address: AdvertisedAddress,

    /// Address to bind for the Node server. Default is `0.0.0.0:5122`
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub bind_address: BindAddress,

    /// Address that other nodes will use to connect to this node. Default is `http://127.0.0.1:5122/`
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub advertised_address: AdvertisedAddress,

    /// # Shutdown grace timeout
    ///
    /// This timeout is used when shutting down the various Restate components to drain all the internal queues.
    ///
    /// Can be configured using the [`humantime`](https://docs.rs/humantime/latest/humantime/fn.parse_duration.html) format.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub shutdown_timeout: Duration,

    /// # Default async runtime thread pool
    ///
    /// Size of the default thread pool used to perform internal tasks.
    /// If not set, it defaults to the number of CPU cores.
    #[builder(setter(strip_option))]
    pub default_thread_pool_size: Option<usize>,

    /// # Tracing Endpoint
    ///
    /// Specify the tracing endpoint to send traces to.
    /// Traces will be exported using [OTLP gRPC](https://opentelemetry.io/docs/specs/otlp/#otlpgrpc)
    /// through [opentelemetry_otlp](https://docs.rs/opentelemetry-otlp/0.12.0/opentelemetry_otlp/).
    ///
    /// To configure the sampling, please refer to the [opentelemetry autoconfigure docs](https://github.com/open-telemetry/opentelemetry-java/blob/main/sdk-extensions/autoconfigure/README.md#sampler).
    pub tracing_endpoint: Option<String>,

    /// # Distributed Tracing JSON Export Path
    ///
    /// If set, an exporter will be configured to write traces to files using the Jaeger JSON format.
    /// Each trace file will start with the `trace` prefix.
    ///
    /// If unset, no traces will be written to file.
    ///
    /// It can be used to export traces in a structured format without configuring a Jaeger agent.
    ///
    /// To inspect the traces, open the Jaeger UI and use the Upload JSON feature to load and inspect them.
    pub tracing_json_path: Option<String>,

    /// # Tracing Filter
    ///
    /// Distributed tracing exporter filter.
    /// Check the [`RUST_LOG` documentation](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) for more details how to configure it.
    pub tracing_filter: String,
    /// # Logging Filter
    ///
    /// Log filter configuration. Can be overridden by the `RUST_LOG` environment variable.
    /// Check the [`RUST_LOG` documentation](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) for more details how to configure it.
    pub log_filter: String,

    /// # Logging format
    ///
    /// Format to use when logging.
    pub log_format: LogFormat,

    /// # Disable ANSI in log output
    ///
    /// Disable ANSI terminal codes for logs. This is useful when the log collector doesn't support processing ANSI terminal codes.
    pub log_disable_ansi_codes: bool,

    /// Timeout for idle histograms.
    ///
    /// The duration after which a histogram is considered idle and will be removed from
    /// metric responses to save memory. Unsetting means that histograms will never be removed.
    #[serde(with = "serde_with::As::<Option<serde_with::DisplayFromStr>>")]
    #[cfg_attr(feature = "schemars", schemars(with = "Option<String>"))]
    pub histogram_inactivity_timeout: Option<humantime::Duration>,

    #[serde(flatten)]
    pub service_client: ServiceClientOptions,

    /// Disable prometheus metric recording and reporting. Default is `false`.
    pub disable_prometheus: bool,

    /// RocksDb general settings and memory limits
    #[serde(flatten)]
    pub rocksdb: RocksDbOptions,
}

impl CommonOptions {
    pub fn shutdown_grace_period(&self) -> std::time::Duration {
        self.shutdown_timeout.into()
    }
}

impl Default for CommonOptions {
    fn default() -> Self {
        Self {
            roles: EnumSet::all(),
            node_name: hostname::get()
                .map(|h| h.into_string().expect("hostname is valid unicode"))
                .unwrap_or("localhost".to_owned()),
            force_node_id: None,
            cluster_name: "localcluster".to_owned(),
            // boot strap the cluster by default. This is very likely to change in the future to be
            // false by default. For now, this is true to make the converged deployment backward
            // compatible and easy for users.
            allow_bootstrap: true,
            base_dir: std::env::current_dir()
                .unwrap()
                .join(DEFAULT_STORAGE_DIRECTORY),
            metadata_store_address: "http://127.0.0.1:5123"
                .parse()
                .expect("valid metadata store address"),
            bind_address: "0.0.0.0:5122".parse().unwrap(),
            advertised_address: AdvertisedAddress::from_str("http://127.0.0.1:5122/").unwrap(),
            histogram_inactivity_timeout: None,
            disable_prometheus: false,
            service_client: Default::default(),
            shutdown_timeout: std::time::Duration::from_secs(60).into(),
            tracing_endpoint: None,
            tracing_json_path: None,
            tracing_filter: "info".to_owned(),
            log_filter: "warn,restate=info".to_string(),
            log_format: Default::default(),
            log_disable_ansi_codes: false,
            default_thread_pool_size: None,
            rocksdb: Default::default(),
        }
    }
}

/// # Service Client options
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, derive_builder::Builder)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "schemars",
    schemars(rename = "ServiceClientOptions", default)
)]
#[builder(default)]
#[derive(Default)]
pub struct ServiceClientOptions {
    #[serde(flatten)]
    pub http: HttpOptions,
    #[serde(flatten)]
    pub lambda: AwsOptions,
}

/// # Log format
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[derive(Debug, Clone, Copy, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum LogFormat {
    /// # Pretty
    ///
    /// Enables verbose logging. Not recommended in production.
    #[default]
    Pretty,
    /// # Compact
    ///
    /// Enables compact logging.
    Compact,
    /// # Json
    ///
    /// Enables json logging. You can use a json log collector to ingest these logs and further process them.
    Json,
}
