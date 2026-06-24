use std::sync::Arc;

use axum::{
    Router,
    body::to_bytes,
    extract::{Request, State},
    http::{Extensions, StatusCode},
    response::{Html, IntoResponse, Json, Redirect, Response},
    routing::get,
};
use juniper::{
    Context, DefaultScalarValue, EmptySubscription, GraphQLType, GraphQLValueAsync, RootNode,
    http::{GraphQLRequest, GraphQLResponse},
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

use crate::config::Config;

/// The contents of the playground's HTML page.
const GRAPHIQL_HTML: &str = include_str!("../data/graphiql.html");

struct AppState<Ctx, Q, M>
where
    Ctx: Context,
    Q: GraphQLType<DefaultScalarValue>,
    M: GraphQLType<DefaultScalarValue>,
{
    schema: RootNode<Q, M, EmptySubscription<Ctx>, DefaultScalarValue>,
    context_fn: Arc<dyn Fn(&Extensions) -> Ctx + Send + Sync>,
}

pub struct Server<Ctx, Q, M>
where
    Ctx: Context,
    Q: GraphQLType<DefaultScalarValue>,
    M: GraphQLType<DefaultScalarValue>,
{
    config: Config,
    state: Arc<AppState<Ctx, Q, M>>,
}

impl<Ctx, Q, M> Clone for Server<Ctx, Q, M>
where
    Ctx: Context,
    Q: GraphQLType<DefaultScalarValue>,
    M: GraphQLType<DefaultScalarValue>,
{
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: self.state.clone(),
        }
    }
}

impl<Ctx, Q, M> Server<Ctx, Q, M>
where
    Ctx: Context + Clone + Send + Sync + 'static,
    Q: GraphQLType<DefaultScalarValue, Context = Ctx> + GraphQLValueAsync + Send + Sync + 'static,
    Q::TypeInfo: Send + Sync,
    M: GraphQLType<DefaultScalarValue, Context = Ctx> + GraphQLValueAsync + Send + Sync + 'static,
    M::TypeInfo: Send + Sync,
{
    pub fn new(
        config: Config,
        schema: RootNode<Q, M, EmptySubscription<Ctx>, DefaultScalarValue>,
        context_fn: impl Fn(&Extensions) -> Ctx + Send + Sync + 'static,
    ) -> Self {
        Self {
            config,
            state: Arc::new(AppState {
                schema,
                context_fn: Arc::new(context_fn),
            }),
        }
    }

    pub fn build(&self) -> Router {
        let state = self.state.clone();

        Router::new()
            .route(&self.config.playground_path, get(playground_handler))
            .route(
                &self.config.graphql_path,
                get(|| async { Redirect::to("/playground") }).post(graphql_handler::<Ctx, Q, M>),
            )
            .layer(
                TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<_>| {
                    let request_id = Uuid::new_v4();
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                        request_id = %request_id,
                    )
                }),
            )
            .layer(CorsLayer::permissive())
            .with_state(state)
    }

    pub async fn serve(self) -> std::io::Result<()> {
        let app = self.build();
        let addr = format!("{}:{}", self.config.host, self.config.port);
        tracing::info!("starting server on {}", addr);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn graphql_handler<Ctx, Q, M>(
    State(state): State<Arc<AppState<Ctx, Q, M>>>,
    req: Request,
) -> Result<Json<GraphQLResponse<DefaultScalarValue>>, Response>
where
    Ctx: Context + Sync,
    Q: GraphQLType<DefaultScalarValue, Context = Ctx> + GraphQLValueAsync + Sync,
    Q::TypeInfo: Sync,
    M: GraphQLType<DefaultScalarValue, Context = Ctx> + GraphQLValueAsync + Sync,
    M::TypeInfo: Sync,
{
    let (parts, body) = req.into_parts();

    let body_bytes = to_bytes(body, 2 * 1024 * 1024)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()).into_response())?;

    let gql_req: GraphQLRequest<DefaultScalarValue> = serde_json::from_slice(&body_bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()).into_response())?;

    let ctx = (state.context_fn)(&parts.extensions);
    let res = gql_req.execute(&state.schema, &ctx).await;
    Ok(Json(res))
}

/// Provides a GraphiQL playground
async fn playground_handler() -> impl IntoResponse {
    Html(GRAPHIQL_HTML)
}
