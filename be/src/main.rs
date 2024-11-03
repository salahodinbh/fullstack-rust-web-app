#[macro_use] extern crate rocket;

use rocket::serde::{ Deserialize, Serialize, json::Json };
use rocket::{ State, response::status::Custom, http::Status };
use tokio_postgres::{ Client, NoTls };
use rocket_cors::{ CorsOptions, AllowedOrigins };

#[derive(Serialize, Deserialize, Clone)]
struct Customer{
    id: Option<i32>,
    name: String,
    email: String,
}

#[post("/api/customers", data = "<customer>")]
async fn add_customer(
    conn: &State<Client>,
    customer: Json<Customer>
) -> Result<Json<Vec<Customer>>, Custom<String>> {
    execute_query(
        conn,
        "INSERT INTO customers (name, email) VALUES ($1, $2)",
        &[&customer.name, &customer.email]
    ).await?;
    get_customers(conn).await
}

#[get("/api/customers")]
async fn get_customers(conn: &State<Client>) -> Result<Json<Vec<Customer>>, Custom<String>> {
    get_customers_from_db(conn).await.map(Json)
}

#[put("/api/customers/<id>", data = "<customer>")]
async fn update_customer(
    conn: &State<Client>,
    id: i32,
    customer: Json<Customer>
) -> Result<Json<Vec<Customer>>, Custom<String>> {
    execute_query(
        conn,
        "UPDATE customers SET name = $1, email = $2 WHERE id = $3",
        &[&customer.name, &customer.email, &id]
    ).await?;
    get_customers(conn).await
}

#[delete("/api/customers/<id>")]
async fn delete_customer(conn: &State<Client>, id: i32) -> Result<Status, Custom<String>> {
    execute_query(conn, "DELETE FROM customers WHERE id = $1", &[&id]).await?;
    Ok(Status::NoContent)
}

async fn get_customers_from_db(client: &Client) -> Result<Vec<Customer>, Custom<String>> {
    let customers = client
        .query("SELECT id, name, email FROM customers", &[]).await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .iter()
        .map(|row| Customer { id: Some(row.get(0)), name: row.get(1), email: row.get(2) })
        .collect::<Vec<Customer>>();

    Ok(customers)
}

async fn execute_query(
    client: &Client,
    query: &str,
    params: &[&(dyn tokio_postgres::types::ToSql + Sync)]
) -> Result<u64, Custom<String>> {
    client
        .execute(query, params).await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

#[launch]
async fn rocket() -> _ {
    let (client, connection) = tokio_postgres
        ::connect("host=localhost user=postgres password=postgres dbname=postgres", NoTls).await
        .expect("Failed to connect to Postgres");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Failed to connect to Postgres: {}", e);
        }
    });

    client
        .execute(
            "CREATE TABLE IF NOT EXISTS customers (
                id SERIAL PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT NOT NULL
            )",
            &[]
        ).await
        .expect("Failed to create table");

    let cors = CorsOptions::default()
        .allowed_origins(AllowedOrigins::all())
        .to_cors()
        .expect("Error while building CORS");

    rocket
        ::build()
        .manage(client)
        .mount("/", routes![add_customer, get_customers, update_customer, delete_customer])
        .attach(cors)
}