use rocket::response::status::BadRequest;
use rocket::serde::json::Json;
use rocket::State;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::sync::Mutex;

#[macro_use]
extern crate rocket;

#[derive(Deserialize, Debug, Serialize)]
struct Error {
    message: String,
}

#[derive(Deserialize, Debug, Serialize, Clone)]
struct Book {
    isbn: String,
    name: String,
    author: String,
}

struct Books(Vec<Book>);

impl Books {
    fn new() -> Self {
        Self(load_books("books.json"))
    }

    fn get_all(&self) -> Vec<Book> {
        self.0.clone()
    }

    fn find(&self, isbn: String) -> Result<Book, Error> {
        for book in &self.0 {
            if isbn == book.isbn {
                return Ok(book.clone());
            }
        }

        Err(Error {
            message: format!("Book with ISBN {} not found", isbn),
        })
    }

    fn add(&mut self, book: Book) -> Result<(), Error> {
        let isbn = book.isbn.clone();
        match self.find(isbn) {
            Ok(_) => {
                return Err(Error {
                    message: "Book already exists in DB".to_string(),
                });
            }
            Err(_) => {
                self.0.push(book);
                save_books("books.json", self.0.clone());
                Ok(())
            }
        }
    }

    fn remove(&mut self, isbn: &str) -> Result<(), Error> {
        match self.find(isbn.to_string()) {
            Ok(book) => {
                let index = self.0.iter().position(|b| b.isbn == book.isbn).unwrap();
                self.0.remove(index);
                save_books("books.json", self.0.clone());
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn search(&self, name: &str) -> Result<Vec<Book>, Error> {
        let found: Vec<Book> = self
            .0
            .iter()
            .filter(|book| book.name.contains(name))
            .cloned()
            .collect();

        if found.is_empty() {
            return Err(Error {
                message: "No book found".to_string(),
            });
        }

        Ok(found)
    }
}

fn load_books(file_path: &str) -> Vec<Book> {
    let data = fs::read_to_string(file_path).unwrap();
    serde_json::from_str(&data).unwrap()
}

fn save_books(file_path: &str, books: Vec<Book>) {
    let data = serde_json::to_string(&books).unwrap();
    match fs::write(file_path, data) {
        Ok(_) => {}
        Err(e) => {
            println!("Error writing file: {}", e);
        }
    }
}

async fn get_book(isbn: &str) -> Book {
    let url = format!(
        "https://www.googleapis.com/books/v1/volumes?q=isbn:{}",
        isbn
    );
    let response = reqwest::get(&url).await.unwrap().text().await.unwrap();

    let data: Value = serde_json::from_str(&response).unwrap();
    let items = data["items"].as_array().unwrap();
    let item = &items[0];
    let volume_info = item["volumeInfo"].as_object().unwrap();

    let name = volume_info["title"].as_str().unwrap();
    let author = volume_info["authors"][0].as_str().unwrap();

    Book {
        isbn: isbn.to_string(),
        name: name.to_string(),
        author: author.to_string(),
    }
}

#[get("/")]
fn get_all(books: &State<Mutex<Books>>) -> Json<Vec<Book>> {
    Json(books.lock().unwrap().get_all())
}

#[get("/get/<isbn>")]
fn get(books: &State<Mutex<Books>>, isbn: &str) -> Result<Json<Book>, BadRequest<Json<Error>>> {
    match books.lock().unwrap().find(isbn.to_string()) {
        Ok(book) => Ok(Json(book)),
        Err(error) => Err(BadRequest(Some(Json(error)))),
    }
}

#[get("/search/<q>")]
fn search(
    books: &State<Mutex<Books>>,
    q: &str,
) -> Result<Json<Vec<Book>>, BadRequest<Json<Error>>> {
    match books.lock().unwrap().search(q) {
        Ok(found) => Ok(Json(found)),
        Err(error) => Err(BadRequest(Some(Json(error)))),
    }
}

#[post("/add", data = "<isbn>")]
async fn add(books: &State<Mutex<Books>>, isbn: &str) -> String {
    let book = get_book(isbn).await;
    match books.lock().unwrap().add(book) {
        Ok(()) => {
            return "Success".to_string();
        }
        Err(error) => {
            return error.message;
        }
    }
}

#[post("/remove", data = "<isbn>")]
fn remove(books: &State<Mutex<Books>>, isbn: &str) -> String {
    match books.lock().unwrap().remove(isbn) {
        Ok(()) => "Success".to_string(),
        Err(e) => e.message,
    }
}

#[launch]
fn rocket() -> _ {
    let books = Books::new();

    rocket::build()
        .manage(Mutex::new(books))
        .mount("/", routes![get_all, get, add, remove, search])
}
