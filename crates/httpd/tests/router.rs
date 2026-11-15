use httpd::{Method, Request, Response, Router, StatusCode};
use serde::Deserialize;

fn router() -> Router {
    let mut r = Router::new();
    r.get("/scenes/{id}", (), |_, req: Request| async move {
        Response::text(200, format!("scene {}", req.param("id")))
    });
    r.get("/scenes/active", (), |_, _| async {
        Response::text(200, "active")
    });
    r.put("/scenes/active/layers/reorder", (), |_, _| async {
        Response::empty(204)
    });
    r.patch(
        "/scenes/active/layers/{id}",
        (),
        |_, req: Request| async move { Response::text(200, format!("layer {}", req.param("id"))) },
    );
    r.post("/count", 41u32, |n, _| async move {
        Response::text(200, (n + 1).to_string())
    });
    r
}

fn text(resp: &Response) -> String {
    String::from_utf8(resp.body_bytes().to_vec()).unwrap()
}

#[test]
fn literal_segments_beat_params_regardless_of_registration_order() {
    let r = router();
    let resp = smol::block_on(r.call(Request::get("/scenes/active")));
    assert_eq!(text(&resp), "active");
    let resp = smol::block_on(r.call(Request::get("/scenes/abc")));
    assert_eq!(text(&resp), "scene abc");
}

#[test]
fn method_is_part_of_the_match() {
    let r = router();
    let resp = smol::block_on(r.call(Request::put("/scenes/active/layers/reorder")));
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = smol::block_on(r.call(Request::patch("/scenes/active/layers/reorder")));
    assert_eq!(text(&resp), "layer reorder");
    let resp = smol::block_on(r.call(Request::delete("/scenes/active")));
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[test]
fn unknown_path_is_404_and_trailing_slash_is_ignored() {
    let r = router();
    assert_eq!(
        smol::block_on(r.call(Request::get("/nope"))).status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        text(&smol::block_on(r.call(Request::get("/scenes/active/")))),
        "active"
    );
}

#[test]
fn params_are_percent_decoded_and_query_is_split_off() {
    let r = router();
    let resp = smol::block_on(r.call(Request::get("/scenes/a%20b?x=1")));
    assert_eq!(text(&resp), "scene a b");
}

#[test]
fn handlers_receive_their_state() {
    let r = router();
    assert_eq!(text(&smol::block_on(r.call(Request::post("/count")))), "42");
}

#[derive(Deserialize, Debug, PartialEq)]
struct Body {
    name: String,
}

#[test]
fn json_body_parses_or_is_a_bad_request() {
    let ok = Request::post("/x").with_json(&serde_json::json!({"name": "n"}));
    assert_eq!(ok.json::<Body>().unwrap(), Body { name: "n".into() });

    let bad = Request::post("/x").with_body(b"{".to_vec(), "application/json");
    let err = bad.json::<Body>().unwrap_err();
    assert_eq!(httpd::IntoResponse::into_response(err).status(), 400);
}

#[derive(Deserialize, Debug, PartialEq)]
struct Auth {
    redirect_uri: String,
    state: String,
}

#[test]
fn query_and_form_decode_urlencoding() {
    let req = Request::get("/oauth/authorize?redirect_uri=https%3A%2F%2Fx.y%2Fcb&state=s%201");
    assert_eq!(
        req.query::<Auth>().unwrap(),
        Auth {
            redirect_uri: "https://x.y/cb".into(),
            state: "s 1".into()
        }
    );
    let form = Request::post("/t").with_body(
        b"redirect_uri=a&state=b".to_vec(),
        "application/x-www-form-urlencoded",
    );
    assert_eq!(form.form::<Auth>().unwrap().state, "b");
}

#[test]
fn ok_and_err_results_convert_to_responses() {
    let ok: Result<Response, Response> = Ok(Response::empty(201));
    assert_eq!(httpd::IntoResponse::into_response(ok).status(), 201);
    let err: Result<Response, Response> = Err(Response::empty(403));
    assert_eq!(httpd::IntoResponse::into_response(err).status(), 403);
}

#[test]
fn method_parses_from_wire_names() {
    assert_eq!(Method::parse("PATCH"), Method::Patch);
    assert_eq!(Method::parse("OPTIONS"), Method::Other);
}
