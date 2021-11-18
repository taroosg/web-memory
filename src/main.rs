use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Error, Request, Response, Server, StatusCode};
use std::{convert::Infallible, net::SocketAddr, str, sync::Arc};
use tera::{Context, Tera};
// データ型のインポート
use serde::Deserialize;
use uuid::Uuid;

use rusqlite::{params, Connection, OptionalExtension};
use tokio::sync::Mutex;

// 自作テンプレートの定義
static TEMPLATE: &str = "Hello, {{name}}!";
// static DBTEMPLATE: &str = "id={{id}}, title={{title}}, content={{content}}";

// リクエストから必要な情報を取り出す構造体の定義
// 参照で取り出すため新たなメモリの確保を必要としない点がポイント
#[derive(Deserialize)]
struct NewPost<'a> {
  title: &'a str,
  content: &'a str,
}

struct Post {
  id: Uuid,
  title: String,
  content: String,
}

impl Post {
  // 投稿を文字列にレンダリングする関数
  fn render(&self, tera: Arc<Tera>) -> String {
    let mut ctx = Context::new();
    ctx.insert("id", &self.id);
    ctx.insert("title", &self.title);
    ctx.insert("content", &self.content);
    tera.render("post", &ctx).unwrap()
  }
}

// fn get_id(req: &Request<Body>) -> Uuid {
//   let body = hyper::body::to_bytes(req.into_body()).await.unwrap();
//   let body = str::from_utf8(&body).unwrap();
//   Uuid::parse_str(body.strip_prefix("post_id=").unwrap()).unwrap()
// }

// idから投稿を探す関数
async fn find_post(
  req: Request<Body>,
  tera: Arc<Tera>,
  conn: Arc<Mutex<Connection>>,
) -> Result<Response<Body>, Error> {
  let body = hyper::body::to_bytes(req.into_body()).await.unwrap();
  let body = str::from_utf8(&body).unwrap();
  let id = Uuid::parse_str(body.strip_prefix("post_id=").unwrap()).unwrap();
  let post = conn
    .lock()
    .await
    .query_row(
      "SELECT id, title, content FROM posts WHERE id=?1",
      params![id],
      |row| {
        Ok(Post {
          id: row.get(0)?,
          title: row.get(1)?,
          content: row.get(2)?,
        })
      },
    )
    .optional()
    .unwrap();
  match post {
    Some(post) => Ok(Response::new(post.render(tera).into())),
    None => Ok(
      Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap(),
    ),
  }
}

// DBにデータを作成する関数
async fn create_post(
  req: Request<Body>,
  _: Arc<Tera>,
  // 排他制御されたDB接続
  // spliteはシングルスレッド動作
  conn: Arc<Mutex<Connection>>,
) -> Result<Response<Body>, Error> {
  // リクエストボディからバイト列のみを取り出す
  let body = hyper::body::to_bytes(req.into_body()).await?;
  // フォームデータのみを取り出す
  let new_post = serde_urlencoded::from_bytes::<NewPost>(&body).unwrap();
  // uuidを生成する
  let id = Uuid::new_v4();
  conn
    // ロックは処理終了時に自動で解除される
    .lock()
    .await
    .execute(
      "INSERT INTO posts(id, title, content) VALUES (?1,?2,?3)",
      // 参照を使ってデータを作成するのでメモリアロケーションは発生しない
      params![&id, &new_post.title, &new_post.content],
    )
    .unwrap();
  Ok(Response::new(id.to_string().into()))
}

// リクエストに対して固定文字列のレスポンスを返す関数
async fn handle(_: Request<Body>) -> Result<Response<Body>, Infallible> {
  Ok(Response::new("Hello World".into()))
}

// テンプレートを使用してリクエストの文字列をレスポンスに組み込む関数
async fn handle_with_body(req: Request<Body>, tera: Arc<Tera>) -> Result<Response<Body>, Error> {
  // bodyからバイト列のみを抽出する．
  let body = hyper::body::to_bytes(req.into_body()).await?;
  // バイト列を文字列として解釈する（参照のみ）．
  let body = str::from_utf8(&body).unwrap();
  // name=の部分を指定して抽出する（参照のみ）．
  let name = body.strip_prefix("name=").unwrap();

  // // 良くない感じにテンプレートでレスポンスを構成する．
  // // 新規テンプレートの作成
  // let mut tera = Tera::default();
  // // helloという名前で定義したテンプレートを呼び出す
  // tera.add_raw_template("hello", TEMPLATE).unwrap();
  // // 新規コンテキストの作成（毎回必要）
  // let mut ctx = Context::new();
  // // コンテキストにnameという名前でリクエストボディのnameの値を入れる
  // ctx.insert("name", name);
  // // helloテンプレートにコンテキストを適用する（毎回必要）
  // let rendered = tera.render("hello", &ctx).unwrap();
  // // レスポンスにテンプレートを使用する
  // Ok(Response::new(rendered.into()))

  // いい感じにテンプレートでレスポンスを返す
  // 新規コンテキストの作成（毎回必要）
  let mut ctx = Context::new();
  // コンテキストにnameという名前でリクエストボディのnameの値を入れる
  ctx.insert("name", name);
  // helloテンプレートにコンテキストを適用する（毎回必要）
  let rendered = tera.render("hello", &ctx).unwrap();
  // レスポンスにテンプレートを使用する
  Ok(Response::new(rendered.into()))
}

async fn route(
  req: Request<Body>,
  tera: Arc<Tera>,
  conn: Arc<Mutex<Connection>>,
) -> Result<Response<Body>, Error> {
  match (req.uri().path(), req.method().as_str()) {
    ("/", "GET") => handle_with_body(req, tera).await,
    // 固定文字列のレスポンスを返す関数を実行
    ("/", _) => handle(req).await.map_err(|e| match e {}),
    ("/posts", "POST") => create_post(req, tera, conn).await,
    (path, "GET") if path.starts_with("/posts/") => find_post(req, tera, conn).await,
    _ => Ok(
      Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap(),
    ),
  }
}

#[tokio::main]
async fn main() {
  let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

  // teraのアロケーションはサーバ立ち上げ時に1回必要なのみ
  // 新規テンプレートの作成
  let mut tera = Tera::default();
  // helloという名前で定義したテンプレートを呼び出す
  tera.add_raw_template("hello", TEMPLATE).unwrap();
  // postという名前で定義したテンプレートを呼び出す
  tera
    .add_raw_template("post", "id: {{id}}\ntitle: {{title}}\ncontent: {{content}}")
    .unwrap();
  let tera = Arc::new(tera);

  // DB接続関連の処理
  let conn = Connection::open_in_memory().unwrap();
  let conn = Arc::new(Mutex::new(conn));

  conn
    .lock()
    .await
    .execute(
      "CREATE TABLE posts (
    id BLOB PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL
  )",
      [],
    )
    .unwrap();

  let make_svc = make_service_fn(|_conn| {
    // Arcを使うとコピーやアロケーションなしでcloneが使用できる
    // cloneはスレッドの数だけ実行される
    let tera = tera.clone();
    let conn = conn.clone();
    async {
      Ok::<_, Infallible>(service_fn(move |req| {
        //  ここでもcloneする．cloneは非同期ランタイムの実行スケジュール単位の数だけ実行される
        route(req, tera.clone(), conn.clone())
      }))
    }
  });
  let server = Server::bind(&addr).serve(make_svc);
  if let Err(e) = server.await {
    eprintln!("server error {}", e)
  }
}
