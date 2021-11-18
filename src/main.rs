use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Method, Request, Response, Server};
use std::{convert::Infallible, net::SocketAddr, str};
use tera::{Context, Tera};
// 自作テンプレートの定義
static TEMPLATE: &str = "Hello, {{name}}!";

// リクエストに対して固定文字列のレスポンスを返す関数
async fn handle(_: Request<Body>) -> Result<Response<Body>, Infallible> {
  Ok(Response::new("Hello World".into()))
}

// テンプレートを使用してリクエストの文字列をレスポンスに組み込む関数
async fn handle_with_body(req: Request<Body>) -> Result<Response<Body>, Error> {
  // bodyからバイト列のみを抽出する．
  let body = hyper::body::to_bytes(req.into_body()).await?;
  // バイト列を文字列として解釈する（参照のみ）．
  let body = str::from_utf8(&body).unwrap();
  // name=の部分を指定して抽出する（参照のみ）．
  let name = body.strip_prefix("name=").unwrap();

  // 良くない感じにテンプレートでレスポンスを構成する．
  // 新規テンプレートの作成
  let mut tera = Tera::default();
  // helloという名前で定義したテンプレートを呼び出す
  tera.add_raw_template("hello", TEMPLATE).unwrap();
  // 新規コンテキストの作成
  let mut ctx = Context::new();
  // コンテキストにnameという名前でリクエストボディのnameの値を入れる
  ctx.insert("name", name);
  // helloテンプレートにコンテキストを適用する
  let rendered = tera.render("hello", &ctx).unwrap();
  // レスポンスにテンプレートを使用する
  Ok(Response::new(rendered.into()))
}

async fn route(req: Request<Body>) -> Result<Response<Body>, Error> {
  match *req.method() {
    Method::POST => handle_with_body(req).await,
    // 固定文字列のレスポンスを返す関数を実行
    _ => handle(req).await.map_err(|e| match e {}),
  }
}

#[tokio::main]
async fn main() {
  let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
  let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(route)) });
  let server = Server::bind(&addr).serve(make_svc);
  if let Err(e) = server.await {
    eprintln!("server error {}", e)
  }
}
