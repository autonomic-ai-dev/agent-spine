use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

use agent_spine::WorkflowDefinition;

#[derive(Debug, Parser)]
#[command(
    name = "agent-spine",
    version,
    about = "Stateful workflow supervision for AI coding agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Display the capabilities planned for the current scaffold.
    Status,
    /// Parse and validate a YAML workflow definition.
    Validate {
        /// Path to a workflow definition file.
        workflow: PathBuf,
    },
    /// Execute a YAML workflow locally.
    Run {
        /// Path to a workflow definition file.
        workflow: PathBuf,
        /// Initial JSON payload.
        #[arg(short, long, default_value = "{}")]
        payload: String,
        /// Path to SQLite database
        #[arg(short, long, default_value = "state.db")]
        db: PathBuf,
    },
    /// Inspect the history of a specific execution.
    Inspect {
        /// Execution ID to inspect.
        execution_id: String,
        /// Path to SQLite database
        #[arg(short, long, default_value = "state.db")]
        db: PathBuf,
    },
    /// Replay an execution to recreate its final state.
    Replay {
        /// Execution ID to replay.
        execution_id: String,
        /// Path to SQLite database
        #[arg(short, long, default_value = "state.db")]
        db: PathBuf,
    },
    /// Serve the Live Dashboard API.
    Serve {
        /// Path to SQLite database
        #[arg(short, long, default_value = "state.db")]
        db: PathBuf,
        /// Port to listen on
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
    },
}

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};

#[tokio::main]
async fn main() {
    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .build()
            .expect("Failed to build OTLP exporter");
        
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .build();
            
        use opentelemetry::trace::TracerProvider;
        let tracer = provider.tracer("agent-spine");
        
        opentelemetry::global::set_tracer_provider(provider);

        let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
        Registry::default()
            .with(EnvFilter::from_default_env())
            .with(tracing_subscriber::fmt::layer())
            .with(telemetry)
            .try_init()
            .ok();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init()
            .ok();
    }

    if let Err(error) = run(Cli::parse().command).await {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

async fn run(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::Status => {
            info!("agent-spine supervisor initialized");
            println!("agent-spine: skeleton ready; workflow validation is available");
            Ok(())
        }
        Command::Validate { workflow } => {
            let validated = WorkflowDefinition::from_path(workflow)?.validate()?;
            info!(
                workflow = validated.definition().name(),
                version = validated.definition().version(),
                nodes = validated.definition().nodes().len(),
                edges = validated.definition().edges().len(),
                "workflow validated"
            );
            println!(
                "validated state-machine workflow '{}' starting at node: {}",
                validated.definition().name(),
                validated.definition().start_node()
            );
            Ok(())
        }
        Command::Run { workflow, payload, db } => {
            let validated = WorkflowDefinition::from_path(workflow)?.validate()?;
            let initial_payload = serde_json::from_str(&payload)?;

            let store = agent_spine::state::SqliteStateStore::new(db)?;
            let store = std::sync::Arc::new(std::sync::Mutex::new(store));

            let supervisor = agent_spine::supervisor::Supervisor::new();
            let router = agent_spine::router::ConfidenceRouter::new(3);

            let mut executor = agent_spine::executor::Executor::new(
                validated,
                store,
                supervisor,
                router,
            );

            let execution_id = executor.run(initial_payload).await?;
            println!("Workflow completed. Execution ID: {}", execution_id);
            Ok(())
        }
        Command::Inspect { execution_id, db } => {
            let store = agent_spine::state::SqliteStateStore::new(db)?;
            let id = std::str::FromStr::from_str(&execution_id)?;
            use agent_spine::WorkflowState;
            let history = store.history(id);

            if history.is_empty() {
                println!("No history found for execution ID {}", id);
            } else {
                for snapshot in history {
                    println!("Sequence: {}", snapshot.sequence());
                    println!("Payload: {}", serde_json::to_string_pretty(snapshot.payload())?);
                    if let Some(trans) = snapshot.transition_edge() {
                        println!("Transition: {} -> {}", trans.from(), trans.to());
                    } else {
                        println!("Transition: Initial");
                    }
                    println!("---");
                }
            }
            Ok(())
        }
        Command::Replay { execution_id, db } => {
            let store = agent_spine::state::SqliteStateStore::new(db)?;
            let id = std::str::FromStr::from_str(&execution_id)?;
            use agent_spine::WorkflowState;
            let history = store.history(id);

            if history.is_empty() {
                println!("No history found for execution ID {}", id);
                return Ok(());
            }

            let mut current_snapshot = history[0].clone();
            println!("Initial Payload: {}", serde_json::to_string_pretty(current_snapshot.payload())?);

            for snapshot in history.into_iter().skip(1) {
                if let Some(trans) = snapshot.transition_edge() {
                    println!("Replaying transition: {} -> {}", trans.from(), trans.to());
                }
                current_snapshot = snapshot;
                println!("New Payload: {}", serde_json::to_string_pretty(current_snapshot.payload())?);
            }

            println!("Replay complete. Final sequence: {}", current_snapshot.sequence());
            Ok(())
        }
        Command::Serve { db, port } => {
            info!("Starting agent-spine gRPC server on port {}", port);

            let store = agent_spine::state::SqliteStateStore::new(db)?;
            let store = std::sync::Arc::new(std::sync::Mutex::new(store));

            let supervisor = agent_spine::supervisor::Supervisor::new();

            let dashboard_api = agent_spine::api::DashboardApi { store };
            let supervisor_api = agent_spine::api::SupervisorApi { supervisor };

            let dashboard_svc =
                agent_spine::api::pb::dashboard_service_server::DashboardServiceServer::new(
                    dashboard_api,
                );
            let supervisor_svc =
                agent_spine::api::pb::supervisor_service_server::SupervisorServiceServer::new(
                    supervisor_api,
                );

            let addr = format!("0.0.0.0:{}", port).parse()?;
            info!("Listening on grpc://{}", addr);

            // Enable gRPC-Web and CORS
            let cors = tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any);

            tonic::transport::Server::builder()
                .accept_http1(true)
                .layer(cors)
                .layer(tonic_web::GrpcWebLayer::new())
                .add_service(dashboard_svc)
                .add_service(supervisor_svc)
                .serve(addr)
                .await?;

            Ok(())
        }
    }
}
