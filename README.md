# Getting Started with SurrealDB and Rust

## A basic CRUD implementation in Rust

### Dependencies

```
[dependencies]
surrealdb = "1.0.0-beta.8"
tokio = { version = "1.21.2", features = ["full"] }
thiserror = "1"
```

### External crates

```rs
use tokio;
use std::collections::BTreeMap;
use surrealdb::{Datastore, Session};
use surrealdb::sql::{Object, Value, Array, thing, parse};
```

### Initialize connection to SurrealDB

```rs
// In memory:
let ds = Datastore::new("memory").await?;

// To file
let ds = Datastore::new("file://temp.db").await?;

// To tikv
let ds = Datastore::new("tikv://127.0.0.1:2379").await?;
```

### Initialize a session

```rs
let ses = Session::for_kv();
```

### Basic query

Check out the [SurrealQueryLanguage Docs](https://surrealdb.com/docs/surrealql)
```rs
let ast = parse(
    "USE NS test DB test;CREATE todo:first_todo SET title = 'Hello, world of SurrealDB with Rust!', body = 'Adding an object and specifying the id';"
    )?;
    let res = ds.process(ast, &ses, None, false).await?;
    println!("{:?}", res)
```
OR
```rs
let sql = 
    "USE NS test DB test;CREATE todo:first_todo SET title = 'Hello, world of SurrealDB with Rust!', body = 'Adding an object and specifying the id';"
    ;
    let res = ds.execute(sql, &ses, None, false).await?;
    println!("{:?}", res)
```

## CRUD methods with a Store struct

### Create a type wrapper for implementing From and TryFrom for surrealdb::Object and surrealdb::Value.

```rs
pub struct W<T>(pub T);

impl TryFrom<W<Value>> for Object {
	type Error = Error;
	fn try_from(val: W<Value>) -> Result<Object, Error> {
		match val.0 {
			Value::Object(obj) => Ok(obj),
			_ => Err(Error::XValueNotOfType("Object")),
		}
	}
}
```


```rs
struct Store {
    ds: Datastore,
    ses: Session
}
```

### Initialize connection

```rs
impl Store {
/* Snip */
pub async fn new() -> Result<Self, Error> {
        let ds = Datastore::new("memory").await?;
		
		let ses = Session::for_db("appns", "appdb");
		
		Ok(Store { ds, ses })
    }
/* Snip */
}
```

### CREATE

```rs
impl Store {
/* Snip */
    pub async fn create(&self) -> Result<String, Error> {
        let sql = "CREATE todo SET title = 'Hello, world!', body = 'Hello, SurrealDB with Rust!'";
        
		let res = self.ds.execute(sql, &self.ses, None, false).await?;
		
		let first_val = res.into_iter().next().map(|r| r.result).expect("id not returned")?;
        
		if let Value::Object(mut val) = first_val.first() {
            let id = val.x_take_val::<String>("id")?;
            Ok(id)
        }else {
			Err(Error::StoreFailToCreate(format!("exec_create, nothing returned.")))
		}
    }
/* Snip */
}
```

### GET by variable

Note that variables are inserted as key value pairs to a binary tree map with a key of type String and a value of type surrealdb::Value. 

The key will represent the variable name in the raw SQL where a $ is added to the front. Ex the "id" key, which holds the uid &str is represented by $id in the query.

This binary tree map is added as the vars argument to the execute method.

```rs
impl Store  {
    /* Snip */
    pub async fn get(&self, uid: &str) -> Result<Object, Error> {
        let sql = "SELECT * FROM todo WHERE id = $id";
        
		let mut vars: BTreeMap<String, Value> = BTreeMap::from([(
            "id".into(), thing(uid)?.into()
        )]);

        let res = self.ds.execute(sql, &self.ses, Some(vars), true).await?;
        
		let first_res = res.into_iter().next().expect("Did not get a response!");
        
		W(first_res.result?.first()).try_into()
    }
    /* Snip */
}
```

### CREATE 

```rs
impl Store  {
    /* Snip */
    pub async fn create(&self) -> Result<String, Error> {
        let sql = "CREATE todo SET title = 'Hello, world!', body = 'Hello, SurrealDB with Rust!'";
        
		let res = self.ds.execute(sql, &self.ses, None, false).await?;
		
		let first_val = res.into_iter().next().map(|r| r.result).expect("id not returned")?;
        
		if let Value::Object(mut val) = first_val.first() {
            let id = val.x_take_val::<String>("id")?;
            Ok(id)
        }else {
			Err(Error::StoreFailToCreate(format!("exec_create, nothing returned.")))
		}
    }
    /* Snip */
}
```

### UPDATE 

```rs
impl Store  {
    /* Snip */
	pub async fn update(&self, tid: &str) -> Result<String, Error> {
		let sql = "UPDATE $th MERGE { body: 'An Updated message!', title: 'Updated!' } RETURN id";
		
		let mut vars: BTreeMap<String, Value> = BTreeMap::from([(
            "th".into(), thing(tid)?.into(),
			
        )]);
        
		let res = self.ds.execute(sql, &self.ses, Some(vars), true).await?;
		
		let first_res = res.into_iter().next().expect("id not returned");
        
		let result = first_res.result?;

		if let Value::Object(mut val) = result.first() {
			val.x_take_val::<String>("id")
		} else {
			Err(Error::StoreFailToCreate(format!("exec_merge {tid}, nothing returned.")))
		}
    }
    /* Snip */
}
```

### DELETE 

```rs
impl Store  {
    /* Snip */
	pub async fn delete(&self, tid: &str) -> Result<String, Error> {
		let sql = "DELETE $th";

		let vars = BTreeMap::from([("th".into(), thing(tid)?.into())]);

		let ress = self.ds.execute(sql, &self.ses, Some(vars), false).await?;

		let first_res = ress.into_iter().next().expect("Did not get a response");

		first_res.result?;

		Ok(tid.to_string())
    }
    /* Snip */
}
```

### GET a list of items

```rs
impl Store  {
    /* Snip */
    pub async fn get_list(&self) -> Result<Vec<Object>, Error> {
        let sql = "SELECT * FROM todo";

        let res = self.ds.execute(sql, &self.ses, None, true).await?;
        
		let first_res = res.into_iter().next().expect("Did not get a response");

		let array: Array = W(first_res.result?).try_into()?;

		array.into_iter().map(|value| W(value).try_into()).collect()
    }
    /* Snip */
}
```

Big thank you to [Jeremy Chone](https://www.youtube.com/c/JeremyChone) and the [Awesome-App](https://awesomeapp.org/) project for the knowledge and inspiration.