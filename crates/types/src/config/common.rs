// Copyright (c) 2023 - 2025 Restate Software, Inc., Restate GmbH.
// All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::num::{NonZeroU16, NonZeroU32, NonZeroUsize};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::Duration;

use enumset::EnumSet;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use restate_serde_util::{NonZeroByteCount, SerdeableHeaderHashMap};

use super::{AwsOptions, HttpOptions, PerfStatsLevel, RocksDbOptions};
use crate::locality::NodeLocation;
use crate::net::{AdvertisedAddress, BindAddress};
use crate::nodes_config::Role;
use crate::retries::RetryPolicy;
use crate::PlainNodeId;

const DEFAULT_STORAGE_DIRECTORY: &str = "restate-data";
const DEFAULT_ADVERTISED_ADDRESS: &str = "http://127.0.0.1:5122/";

static HOSTNAME: LazyLock<String> = LazyLock::new(|| {
    hostname::get()
        .map(|h| h.into_string().expect("hostname is valid unicode"))
        .unwrap_or("INVALID_HOSTANAME".to_owned())
});

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, derive_builder::Builder)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schemars", schemars(default))]
#[serde(rename_all = "kebab-case")]
#[builder(default)]
pub struct CommonOptions {
    /// Defines the roles which this Restate node should run, by default the node
    /// starts with all roles.
    pub roles: EnumSet<Role>,

    /// # Node Name
    ///
    /// Unique name for this node in the cluster. The node must not change unless
    /// it's started with empty local store. It defaults to the node's hostname.
    node_name: Option<String>,

    /// # Node Location
    ///
    /// Setting the location allows Restate to form a tree-like cluster topology.
    /// The value is written in the format of "<region>[.zone]" to assign this node
    /// to a specific region, or to a zone within a region.
    ///
    /// The value of region and zone is arbitrary but whitespace and `.` are disallowed.
    ///
    ///
    /// NOTE: It's _strongly_ recommended to not change the node's location string after
    /// its initial registration. Changing the location may result in data loss or data
    /// inconsistency if `log-server` is enabled on this node.
    ///
    /// When this value is not set, the node is considered to be in the _default_ location.
    /// The _default_ location means that the node is not assigned to any specific region or zone.
    ///
    /// ## Examples
    /// - `us-west` -- the node is in the `us-west` region.
    /// - `us-west.a1` -- the node is in the `us-west` region and in the `a1` zone.
    /// - `` -- [default] the node is in the default location
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    #[builder(setter(strip_option))]
    location: Option<NodeLocation>,

    /// If set, the node insists on acquiring this node ID.
    pub force_node_id: Option<PlainNodeId>,

    /// # Cluster Name
    ///
    /// A unique identifier for the cluster. All nodes in the same cluster should
    /// have the same.
    cluster_name: String,

    /// If true, then a new cluster is bootstrapped. This node *must* have an admin
    /// role and a new nodes configuration will be created that includes this node.
    ///
    /// Bootstrap is allowed by default, unless it is explicitly disabled in config files.
    ///
    /// Default: true
    pub allow_bootstrap: bool,

    /// The working directory which this Restate node should use for relative paths. The default is
    /// `restate-data` under the current working directory.
    #[builder(setter(strip_option))]
    base_dir: Option<PathBuf>,

    #[serde(flatten)]
    pub metadata_store_client: MetadataStoreClientOptions,

    /// Address to bind for the Node server. Derived from the advertised address, defaulting
    /// to `0.0.0.0:$PORT` (where the port will be inferred from the URL scheme).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub bind_address: Option<BindAddress>,

    /// Address that other nodes will use to connect to this node. Default is `http://127.0.0.1:5122/`
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub advertised_address: AdvertisedAddress,

    /// # Partitions
    ///
    /// Number of partitions that will be provisioned during cluster bootstrap,
    /// partitions used to process messages.
    ///
    /// NOTE: This config entry only impacts the initial number of partitions, the
    /// value of this entry is ignored for bootstrapped nodes/clusters.
    ///
    /// Cannot be higher than `65535` (You should almost never need as many partitions anyway)
    pub bootstrap_num_partitions: NonZeroU16,

    /// # Shutdown grace timeout
    ///
    /// This timeout is used when shutting down the various Restate components to drain all the internal queues.
    ///
    /// Can be configured using the [`humantime`](https://docs.rs/humantime/latest/humantime/fn.parse_duration.html) format.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub shutdown_timeout: humantime::Duration,

    /// # Default async runtime thread pool
    ///
    /// Size of the default thread pool used to perform internal tasks.
    /// If not set, it defaults to the number of CPU cores.
    #[builder(setter(strip_option))]
    default_thread_pool_size: Option<usize>,

    #[serde(flatten)]
    pub tracing: TracingOptions,

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

    /// Address to bind for the tokio-console tracing subscriber. If unset and restate-server is
    /// built with tokio-console support, it'll listen on `0.0.0.0:6669`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub tokio_console_bind_address: Option<BindAddress>,

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

    /// Storage high priority thread pool
    ///
    /// This configures the restate-managed storage thread pool for performing
    /// high-priority or latency-sensitive storage tasks when the IO operation cannot
    /// be performed on in-memory caches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_high_priority_bg_threads: Option<NonZeroUsize>,

    /// Storage low priority thread pool
    ///
    /// This configures the restate-managed storage thread pool for performing
    /// low-priority or latency-insensitive storage tasks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_low_priority_bg_threads: Option<NonZeroUsize>,

    /// # Total memory limit for rocksdb caches and memtables.
    ///
    /// This includes memory for uncompressed block cache and all memtables by all open databases.
    /// The memory size used for rocksdb caches.
    #[serde_as(as = "NonZeroByteCount")]
    #[cfg_attr(feature = "schemars", schemars(with = "NonZeroByteCount"))]
    pub rocksdb_total_memory_size: NonZeroUsize,

    /// # Rocksdb total memtable size ratio
    ///
    /// The memory size used across all memtables (ratio between 0 to 1.0). This
    /// limits how much memory memtables can eat up from the value in rocksdb-total-memory-limit.
    /// When set to 0, memtables can take all available memory up to the value specified
    /// in rocksdb-total-memory-limit. This value will be sanitized to 1.0 if outside the valid bounds.
    rocksdb_total_memtables_ratio: f32,

    /// # Rocksdb Background Threads
    ///
    /// The number of threads to reserve to Rocksdb background tasks. Defaults to the number of
    /// cores on the machine.
    #[serde(skip_serializing_if = "Option::is_none")]
    rocksdb_bg_threads: Option<NonZeroU32>,

    /// # Rocksdb High Priority Background Threads
    ///
    /// The number of threads to reserve to high priority Rocksdb background tasks.
    pub rocksdb_high_priority_bg_threads: NonZeroU32,

    /// # Rocksdb stall detection threshold
    ///
    /// This defines the duration afterwhich a write is to be considered in "stall" state. For
    /// every write that meets this threshold, the system will increment the
    /// `restate.rocksdb_stall_flare` gauge, if the write is unstalled, the guage will be updated
    /// accordingly.
    #[serde(with = "serde_with::As::<serde_with::DisplayFromStr>")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub rocksdb_write_stall_threshold: humantime::Duration,

    /// # Allow rocksdb writes to stall if memory limit is reached
    ///
    /// Note if automatic memory budgeting is enabled, it should be safe to allow rocksdb to stall
    /// if it hits the limit. However, if rocksdb stall kicked in, it's unlikely that the system
    /// will recover from this without intervention.
    pub rocksdb_enable_stall_on_memory_limit: bool,

    /// # Rocksdb performance statistics level
    ///
    /// Defines the level of PerfContext used internally by rocksdb. Default is `enable-count`
    /// which should be sufficient for most users. Note that higher levels incur a CPU cost and
    /// might slow down the critical path.
    pub rocksdb_perf_level: PerfStatsLevel,

    /// RocksDb base settings and memory limits that get applied on every database
    #[serde(flatten)]
    pub rocksdb: RocksDbOptions,

    /// # Metadata update interval
    ///
    /// The interval at which each node checks for metadata updates it has observed from different
    /// nodes or other sources.
    #[serde(with = "serde_with::As::<serde_with::DisplayFromStr>")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub metadata_update_interval: humantime::Duration,

    /// # Network error retry policy
    ///
    /// The retry policy for network related errors
    pub network_error_retry_policy: RetryPolicy,

    /// # Initialization timeout
    ///
    /// The timeout until the node gives up joining a cluster and initializing itself.
    #[serde(with = "serde_with::As::<serde_with::DisplayFromStr>")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub initialization_timeout: humantime::Duration,

    /// # Disable telemetry
    ///
    /// Restate uses Scarf to collect anonymous usage data to help us understand how the software is being used.
    /// You can set this flag to true to disable this collection. It can also be set with the environment variable DO_NOT_TRACK=true.
    pub disable_telemetry: bool,
}

impl CommonOptions {
    pub fn shutdown_grace_period(&self) -> Duration {
        self.shutdown_timeout.into()
    }
    // todo: It's imperative that the node doesn't change its name after start. Move this to a
    // Once lock to ensure it doesn't change over time, even if the physical hostname changes.
    pub fn node_name(&self) -> &str {
        self.node_name.as_ref().unwrap_or(&HOSTNAME)
    }

    /// The node location as defined in the configuration file, or the default configuration if
    /// unset.
    pub fn location(&self) -> &NodeLocation {
        static DEFAULT_LOCATION: NodeLocation = NodeLocation::new();
        self.location.as_ref().unwrap_or(&DEFAULT_LOCATION)
    }

    #[cfg(feature = "unsafe-mutable-config")]
    pub fn set_node_name(&mut self, node_name: impl Into<String>) {
        self.node_name = Some(node_name.into())
    }

    // same as node_name
    pub fn cluster_name(&self) -> &str {
        &self.cluster_name
    }

    #[cfg(feature = "unsafe-mutable-config")]
    pub fn set_cluster_name(&mut self, cluster_name: impl Into<String>) {
        self.cluster_name = cluster_name.into()
    }

    #[cfg(feature = "unsafe-mutable-config")]
    pub fn set_base_dir(&mut self, path: impl Into<PathBuf>) {
        self.base_dir = Some(path.into());
    }

    pub fn base_dir(&self) -> PathBuf {
        self.base_dir.clone().unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap()
                .join(DEFAULT_STORAGE_DIRECTORY)
        })
    }

    pub fn rocksdb_actual_total_memtables_size(&self) -> usize {
        let sanitized = self.rocksdb_total_memtables_ratio.clamp(0.0, 1.0) as f64;
        let total_mem = self.rocksdb_total_memory_size.get() as f64;
        (total_mem * sanitized) as usize
    }

    pub fn rocksdb_safe_total_memtables_size(&self) -> usize {
        // %5 safety margin
        (self.rocksdb_actual_total_memtables_size() as f64 * 0.95).floor() as usize
    }

    pub fn storage_high_priority_bg_threads(&self) -> NonZeroUsize {
        self.storage_high_priority_bg_threads.unwrap_or(
            std::thread::available_parallelism()
                // Shouldn't really fail, but just in case.
                .unwrap_or(NonZeroUsize::new(4).unwrap()),
        )
    }

    pub fn default_thread_pool_size(&self) -> usize {
        self.default_thread_pool_size.unwrap_or(
            std::thread::available_parallelism()
                // Shouldn't really fail, but just in case.
                .unwrap_or(NonZeroUsize::new(4).unwrap())
                .get(),
        )
    }

    pub fn storage_low_priority_bg_threads(&self) -> NonZeroUsize {
        self.storage_low_priority_bg_threads.unwrap_or(
            std::thread::available_parallelism()
                // Shouldn't really fail, but just in case.
                .unwrap_or(NonZeroUsize::new(4).unwrap()),
        )
    }

    pub fn rocksdb_bg_threads(&self) -> NonZeroU32 {
        self.rocksdb_bg_threads.unwrap_or(
            std::thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(3).unwrap())
                .try_into()
                .expect("number of cpu cores fits in u32"),
        )
    }

    /// set derived values if they are not configured to reduce verbose configurations
    pub fn set_derived_values(&mut self) {
        // Only derive bind_address if it is not explicitly set
        if self.bind_address.is_none() {
            self.bind_address = Some(self.advertised_address.derive_bind_address());
        }
    }
}

impl Default for CommonOptions {
    fn default() -> Self {
        Self {
            // todo remove `- Role::Ingress` when the safe rollback version supports ingress
            //   see "roles_compat_test" test below.
            roles: EnumSet::all() - Role::HttpIngress,
            node_name: None,
            location: None,
            force_node_id: None,
            cluster_name: "localcluster".to_owned(),
            // boot strap the cluster by default. This is very likely to change in the future to be
            // false by default. For now, this is true to make the converged deployment backward
            // compatible and easy for users.
            allow_bootstrap: true,
            base_dir: None,
            metadata_store_client: MetadataStoreClientOptions::default(),
            bind_address: None,
            advertised_address: AdvertisedAddress::from_str(DEFAULT_ADVERTISED_ADDRESS).unwrap(),
            bootstrap_num_partitions: NonZeroU16::new(24).expect("is not zero"),
            histogram_inactivity_timeout: None,
            disable_prometheus: false,
            service_client: Default::default(),
            shutdown_timeout: Duration::from_secs(60).into(),
            tracing: TracingOptions::default(),
            log_filter: "warn,restate=info".to_string(),
            log_format: Default::default(),
            log_disable_ansi_codes: false,
            tokio_console_bind_address: Some(BindAddress::Socket("0.0.0.0:6669".parse().unwrap())),
            default_thread_pool_size: None,
            storage_high_priority_bg_threads: None,
            storage_low_priority_bg_threads: None,
            rocksdb_total_memtables_ratio: 0.5, // (50% of rocksdb-total-memory-size)
            rocksdb_total_memory_size: NonZeroUsize::new(6_000_000_000).unwrap(), // 4GB
            rocksdb_bg_threads: None,
            rocksdb_high_priority_bg_threads: NonZeroU32::new(2).unwrap(),
            rocksdb_write_stall_threshold: Duration::from_secs(3).into(),
            rocksdb_enable_stall_on_memory_limit: false,
            rocksdb_perf_level: PerfStatsLevel::EnableCount,
            rocksdb: Default::default(),
            metadata_update_interval: Duration::from_secs(3).into(),
            network_error_retry_policy: RetryPolicy::exponential(
                Duration::from_millis(10),
                2.0,
                Some(15),
                Some(Duration::from_secs(5)),
            ),
            initialization_timeout: Duration::from_secs(5 * 60).into(),
            disable_telemetry: false,
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
#[serde(rename_all = "kebab-case")]
pub struct ServiceClientOptions {
    #[serde(flatten)]
    pub http: HttpOptions,
    #[serde(flatten)]
    pub lambda: AwsOptions,

    /// # Request identity private key PEM file
    ///
    /// A path to a file, such as "/var/secrets/key.pem", which contains exactly one ed25519 private
    /// key in PEM format. Such a file can be generated with `openssl genpkey -algorithm ed25519`.
    /// If provided, this key will be used to attach JWTs to requests from this client which
    /// SDKs may optionally verify, proving that the caller is a particular Restate instance.
    ///
    /// This file is currently only read on client creation, but this may change in future.
    /// Parsed public keys will be logged at INFO level in the same format that SDKs expect.
    pub request_identity_private_key_pem_file: Option<PathBuf>,
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

/// # Service Client options
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, derive_builder::Builder)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "schemars",
    schemars(rename = "MetadataStoreClientOptions", default)
)]
#[builder(default)]
#[serde(rename_all = "kebab-case")]
pub struct MetadataStoreClientOptions {
    /// # Metadata store
    ///
    /// Metadata store server kind.
    pub metadata_store_client: MetadataStoreClient,

    /// # Connect timeout
    ///
    /// TCP connection timeout for connecting to the metadata store.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub metadata_store_connect_timeout: humantime::Duration,

    /// # Metadata Store Keep Alive Interval
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub metadata_store_keep_alive_interval: humantime::Duration,

    /// # Metadata Store Keep Alive Timeout
    #[serde_as(as = "serde_with::DisplayFromStr")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub metadata_store_keep_alive_timeout: humantime::Duration,

    /// # Backoff policy used by the metadata store client
    ///
    /// Backoff policy used by the metadata store client when it encounters concurrent
    /// modifications.
    pub metadata_store_client_backoff_policy: RetryPolicy,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case"
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "schemars",
    schemars(
        title = "Metadata Store",
        description = "Definition of a bootstrap metadata store"
    )
)]
pub enum ObjectStoreCredentials {
    /// # Use standard AWS environment variables
    ///
    /// Configure the object store by setting the standard AWS env variables (prefixed with AWS_)
    AwsEnv,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    try_from = "MetadataStoreClientShadow"
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "schemars",
    schemars(
        title = "Metadata Store",
        description = "Definition of a bootstrap metadata store"
    )
)]
pub enum MetadataStoreClient {
    /// Connects to an embedded metadata store that is run by nodes that run with the MetadataStore role.
    Embedded {
        #[cfg_attr(feature = "schemars", schemars(with = "Vec<String>"))]
        addresses: Vec<AdvertisedAddress>,
    },
    /// Uses external etcd as metadata store.
    /// The addresses are formatted as `host:port`
    Etcd {
        #[cfg_attr(feature = "schemars", schemars(with = "String"))]
        addresses: Vec<String>,
    },
    /// Uses an object store as a metadata store.
    ObjectStore {
        credentials: ObjectStoreCredentials,

        /// # The bucket name to use for storage
        #[cfg_attr(feature = "schemars", schemars(with = "String"))]
        bucket: String,
    },
}

#[derive(Debug, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case"
)]
// TODO(azmy): Remove this Shadow struct once we no longer support
// the `address` configuration param.
enum MetadataStoreClientShadow {
    /// Connects to an embedded metadata store that is run by nodes that run with the MetadataStore role.
    Embedded {
        address: Option<AdvertisedAddress>,
        addresses: Vec<AdvertisedAddress>,
    },
    /// Uses external etcd as metadata store.
    /// The addresses are formatted as `host:port`
    Etcd { addresses: Vec<String> },
    /// Uses an object store as a metadata store.
    ObjectStore {
        credentials: ObjectStoreCredentials,
        /// # The bucket name to use for storage
        bucket: String,
    },
}

impl TryFrom<MetadataStoreClientShadow> for MetadataStoreClient {
    type Error = &'static str;
    fn try_from(value: MetadataStoreClientShadow) -> Result<Self, Self::Error> {
        let result = match value {
            MetadataStoreClientShadow::ObjectStore {
                credentials,
                bucket,
            } => Self::ObjectStore {
                credentials,
                bucket,
            },
            MetadataStoreClientShadow::Etcd { addresses } => Self::Etcd { addresses },
            MetadataStoreClientShadow::Embedded { address, addresses } => {
                let default_address: AdvertisedAddress =
                    DEFAULT_ADVERTISED_ADDRESS.parse().unwrap();

                Self::Embedded {
                    addresses: match address {
                        Some(address) if addresses == vec![default_address] => vec![address],
                        Some(_) => return Err("Conflicting configuration, embedded metadata-store-client cannot have both `address` and `addresses`"),
                        None => addresses,
                    },
                }
            }
        };

        Ok(result)
    }
}

impl Default for MetadataStoreClientOptions {
    fn default() -> Self {
        Self {
            metadata_store_client: MetadataStoreClient::Embedded {
                addresses: vec![DEFAULT_ADVERTISED_ADDRESS
                    .parse()
                    .expect("valid metadata store address")],
            },
            metadata_store_connect_timeout: Duration::from_secs(5).into(),
            metadata_store_keep_alive_interval: Duration::from_secs(40).into(),
            metadata_store_keep_alive_timeout: Duration::from_secs(20).into(),
            metadata_store_client_backoff_policy: RetryPolicy::exponential(
                Duration::from_millis(10),
                2.0,
                None,
                Some(Duration::from_millis(100)),
            ),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "schemars",
    schemars(title = "Tracing", description = "Options for tracing")
)]
pub struct TracingOptions {
    /// # Tracing Endpoint
    ///
    /// This is a shortcut to set both [`Self::tracing_runtime_endpoint`], and [`Self::tracing_services_endpoint`].
    ///
    /// Specify the tracing endpoint to send runtime traces to.
    /// Traces will be exported using [OTLP gRPC](https://opentelemetry.io/docs/specs/otlp/#otlpgrpc)
    /// through [opentelemetry_otlp](https://docs.rs/opentelemetry-otlp/0.12.0/opentelemetry_otlp/).
    ///
    /// To configure the sampling, please refer to the [opentelemetry autoconfigure docs](https://github.com/open-telemetry/opentelemetry-java/blob/main/sdk-extensions/autoconfigure/README.md#sampler).
    pub tracing_endpoint: Option<String>,

    /// # Runtime Tracing Endpoint
    ///
    /// Overrides [`Self::tracing_endpoint`] for runtime traces
    ///
    /// Specify the tracing endpoint to send runtime traces to.
    /// Traces will be exported using [OTLP gRPC](https://opentelemetry.io/docs/specs/otlp/#otlpgrpc)
    /// through [opentelemetry_otlp](https://docs.rs/opentelemetry-otlp/0.12.0/opentelemetry_otlp/).
    ///
    /// To configure the sampling, please refer to the [opentelemetry autoconfigure docs](https://github.com/open-telemetry/opentelemetry-java/blob/main/sdk-extensions/autoconfigure/README.md#sampler).
    pub tracing_runtime_endpoint: Option<String>,

    /// # Services Tracing Endpoint
    ///
    /// Overrides [`Self::tracing_endpoint`] for services traces
    ///
    /// Specify the tracing endpoint to send services traces to.
    /// Traces will be exported using [OTLP gRPC](https://opentelemetry.io/docs/specs/otlp/#otlpgrpc)
    /// through [opentelemetry_otlp](https://docs.rs/opentelemetry-otlp/0.12.0/opentelemetry_otlp/).
    ///
    /// To configure the sampling, please refer to the [opentelemetry autoconfigure docs](https://github.com/open-telemetry/opentelemetry-java/blob/main/sdk-extensions/autoconfigure/README.md#sampler).
    pub tracing_services_endpoint: Option<String>,

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

    /// # Additional tracing headers
    ///
    /// Specify additional headers you want the system to send to the tracing endpoint (e.g.
    /// authentication headers).
    pub tracing_headers: SerdeableHeaderHashMap,
}

impl Default for TracingOptions {
    fn default() -> Self {
        Self {
            tracing_endpoint: None,
            tracing_runtime_endpoint: None,
            tracing_services_endpoint: None,
            tracing_json_path: None,
            tracing_filter: "info".to_owned(),
            tracing_headers: SerdeableHeaderHashMap::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::nodes_config::Role;

    use super::CommonOptions;

    #[test]
    fn roles_compat_test() {
        let opts = CommonOptions::default();
        // make sure we don't add ingress by default until previous version can parse nodes
        // configuration with this role.
        assert!(!opts.roles.contains(Role::HttpIngress));
    }
}
