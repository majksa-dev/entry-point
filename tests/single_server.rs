mod helper;

use essentials::debug;
use gateway::{http::HeaderMapExt, Request};
use helper::*;
use http::{header, Method, StatusCode};
use pretty_assertions::assert_eq;
use testing_utils::macros as utils;

#[utils::test(setup = before_each, teardown = after_each)]
async fn should_succeed(ctx: Context) -> Context {
    let mut request = Request::new("/hello".to_string(), Method::GET);
    request.insert_header(header::HOST, DOMAIN);
    request.insert_header(header::CONTENT_LENGTH, "0");
    let response = run_request(request, &ctx).await;
    debug!("{:?}", response);
    let length = response.get_content_length().unwrap();
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.body().unwrap().read_all(length).await.unwrap(),
        "hello"
    );
    ctx
}

#[utils::test(setup = before_each, teardown = after_each)]
async fn should_succeed_when_calling_1000_times(ctx: Context) -> Context {
    for _ in 0..1000 {
        let mut request = Request::new("/hello".to_string(), Method::GET);
        request.insert_header(header::HOST, DOMAIN);
        request.insert_header(header::CONTENT_LENGTH, "0");
        let response = run_request(request, &ctx).await;
        debug!("{:?}", response);
        let length = response.get_content_length().unwrap();
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.body().unwrap().read_all(length).await.unwrap(),
            "hello"
        );
    }
    ctx
}
