use std::sync::Arc;

use axum::{
    Router,
    extract::{FromRequestParts, Request},
    http::{StatusCode, request::Parts},
    middleware::{Next, from_fn},
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

#[async_trait::async_trait]
pub trait Authenticator: Send + Sync + 'static {
    type User: Clone + Send + Sync;
    type Error: IntoResponse + Send;

    async fn authenticate(&self, parts: &Parts) -> Result<Self::User, Self::Error>;
}

#[derive(Clone)]
pub struct CurrentUser<U> {
    pub user: U,
}

impl<S, U> FromRequestParts<S> for CurrentUser<U>
where
    U: Clone + Send + Sync + 'static,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<U>()
            .cloned()
            .map(|user| CurrentUser { user })
            .ok_or_else(|| StatusCode::UNAUTHORIZED.into_response())
    }
}

pub fn add_auth<U: Clone + Send + Sync + 'static>(
    router: Router,
    auth: impl Authenticator<User = U> + 'static,
) -> Router {
    let auth = Arc::new(auth);
    router.layer(from_fn(move |req: Request, next: Next| {
        let auth = auth.clone();
        async move {
            let (mut parts, body) = req.into_parts();
            if let Ok(user) = auth.authenticate(&parts).await {
                parts.extensions.insert(user);
            }
            let req = Request::from_parts(parts, body);
            next.run(req).await
        }
    }))
}

pub fn require_auth<U: Clone + Send + Sync + 'static>(
    router: Router,
    auth: impl Authenticator<User = U> + 'static,
) -> Router {
    let auth = Arc::new(auth);
    router.layer(from_fn(move |req: Request, next: Next| {
        let auth = auth.clone();
        async move {
            let (mut parts, body) = req.into_parts();
            match auth.authenticate(&parts).await {
                Ok(user) => {
                    parts.extensions.insert(user);
                    let req = Request::from_parts(parts, body);
                    next.run(req).await
                }
                Err(e) => e.into_response(),
            }
        }
    }))
}

#[derive(Clone)]
pub struct CookieAuth {
    cookie_name: String,
    valid_token: String,
    user: String,
}

impl CookieAuth {
    pub fn from_env() -> Self {
        Self {
            cookie_name: std::env::var("AUTH_COOKIE").unwrap_or_else(|_| "session".into()),
            valid_token: std::env::var("AUTH_TOKEN").expect("AUTH_TOKEN must be set"),
            user: std::env::var("AUTH_USER").unwrap_or_else(|_| "admin".into()),
        }
    }

    pub fn cookie_name(&self) -> &str {
        &self.cookie_name
    }

    pub fn valid_token(&self) -> &str {
        &self.valid_token
    }

    pub fn build_session_cookie(&self) -> Cookie<'static> {
        Cookie::build((self.cookie_name.clone(), self.valid_token.clone()))
            .http_only(true)
            .path("/")
            .same_site(SameSite::Strict)
            .build()
    }
}

#[async_trait::async_trait]
impl Authenticator for CookieAuth {
    type User = String;
    type Error = (StatusCode, &'static str);

    async fn authenticate(&self, parts: &Parts) -> Result<Self::User, Self::Error> {
        let jar = CookieJar::from_headers(&parts.headers);

        let cookie = jar
            .get(&self.cookie_name)
            .ok_or((StatusCode::UNAUTHORIZED, "Missing session cookie"))?;

        if cookie.value() == self.valid_token {
            Ok(self.user.clone())
        } else {
            Err((StatusCode::UNAUTHORIZED, "Invalid session"))
        }
    }
}
