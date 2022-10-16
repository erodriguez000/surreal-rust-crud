use tokio;
use std::collections::BTreeMap;
use surrealdb::{Datastore, Session};
use surrealdb::sql::{Object, Value, Array, thing};

#[tokio::main]
async fn main() -> Result<(), Error> {

	// Connect to Database by creating a new Store struct
    let store = Store::new().await?;
    
	// Create an object, return String id
	let new_object_id = store.create().await?;
    println!("{}", new_object_id);

	// Get an object by id, return a surrealdb::Object
    let object = store.get(&new_object_id).await?;
    println!("Fetched Object: {}", object.to_string());

	// Update an object by id, return String id
	let new_object_id = store.create().await?;
    println!("ID to be Updated: {}", new_object_id);

	let updated_object_id = store.update(&new_object_id).await?;
	println!("Updated ID: {}", updated_object_id);
	
	// Delete an object by id, return String id
	let new_object_id = store.create().await?;
    println!("ID to be deleted: {}", new_object_id);

    let deleted_object_id = store.delete(&new_object_id).await?;
    println!("Deleted Object ID: {}", deleted_object_id); 

	// Get a list of items, returns Vec<surrealdb::Object>
	let res = store.get_list().await?;

	// Collect response into a Vec<String>
	let vals: Vec<String> = res.iter().map(|obj| {
		obj.to_string()
	}).collect();

	for obj in vals {
		println!("Object in DB: {}", obj)
	}

    Ok(())
}

// region:      ---- Generic Wrapper Struct for implementing From/TryFrom for type conversions SurrealDB Value <-> Object

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

impl TryFrom<W<Value>> for Array {
	type Error = Error;
	fn try_from(val: W<Value>) -> Result<Array, Error> {
		match val.0 {
			Value::Array(obj) => Ok(obj),
			_ => Err(Error::XValueNotOfType("Array")),
		}
	}
}

impl TryFrom<W<Value>> for i64 {
	type Error = Error;
	fn try_from(val: W<Value>) -> Result<i64, Error> {
		match val.0 {
			Value::Number(obj) => Ok(obj.as_int()),
			_ => Err(Error::XValueNotOfType("i64")),
		}
	}
}

impl TryFrom<W<Value>> for bool {
	type Error = Error;
	fn try_from(val: W<Value>) -> Result<bool, Error> {
		match val.0 {
			Value::False => Ok(false),
			Value::True => Ok(true),
			_ => Err(Error::XValueNotOfType("bool")),
		}
	}
}

impl TryFrom<W<Value>> for String {
	type Error = Error;
	fn try_from(val: W<Value>) -> Result<String, Error> {
		match val.0 {
			Value::Strand(strand) => Ok(strand.as_string()),
			Value::Thing(thing) => Ok(thing.to_string()),
			_ => Err(Error::XValueNotOfType("String")),
		}
	}
}

// endregion:   ---- Generic Wrapper Struct for implementing From/TryFrom for type conversions SurrealDB Value <-> Object

// region:      ---- Surreal DB Object implementations

pub trait XTakeImpl<T> {
	fn x_take_impl(&mut self, k: &str) -> Result<Option<T>, Error>;
}

/// For turbofish friendly version of XTakeInto with blanket implementation.
/// Note: Has a blanket implementation. Not to be implemented directly.
///       XTakeInto is the to be implemented trait
pub trait XTake {
	fn x_take<T>(&mut self, k: &str) -> Result<Option<T>, Error>
	where
		Self: XTakeImpl<T>;
}

/// Blanket implementation
impl<S> XTake for S {
	fn x_take<T>(&mut self, k: &str) -> Result<Option<T>, Error>
	where
		Self: XTakeImpl<T>,
	{
		XTakeImpl::x_take_impl(self, k)
	}
}

/// Take the value and return Error if None.
/// Note: Has a blanket implementation. Not to be implemented directly.
///       XTakeInto is the to be implemented trait
pub trait XTakeVal {
	fn x_take_val<T>(&mut self, k: &str) -> Result<T, Error>
	where
		Self: XTakeImpl<T>;
}

/// Blanket implementation
impl<S> XTakeVal for S {
	fn x_take_val<T>(&mut self, k: &str) -> Result<T, Error>
	where
		Self: XTakeImpl<T>,
	{
		let val: Option<T> = XTakeImpl::x_take_impl(self, k)?;
		val.ok_or_else(|| Error::XPropertyNotFound(k.to_string()))
	}
}


impl XTakeImpl<String> for Object {
	fn x_take_impl(&mut self, k: &str) -> Result<Option<String>, Error> {
		let v = self.remove(k).map(|v| W(v).try_into());
		match v {
			None => Ok(None),
			Some(Ok(val)) => Ok(Some(val)),
			Some(Err(ex)) => Err(ex),
		}
	}
}

impl XTakeImpl<i64> for Object {
	fn x_take_impl(&mut self, k: &str) -> Result<Option<i64>, Error> {
		let v = self.remove(k).map(|v| W(v).try_into());
		match v {
			None => Ok(None),
			Some(Ok(val)) => Ok(Some(val)),
			Some(Err(ex)) => Err(ex),
		}
	}
}

impl XTakeImpl<bool> for Object {
	fn x_take_impl(&mut self, k: &str) -> Result<Option<bool>, Error> {
		Ok(self.remove(k).map(|v| v.is_true()))
	}
}
// endregion:   ---- Surreal DB Object implementations

// region:      ---- Error type enumerator

// enumerate errors to allow use of the ? operator
#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("Fail to get Ctx")]
	CtxFail,

	#[error("Value not of type '{0}'")]
	XValueNotOfType(&'static str),

	#[error("Property '{0}' not found")]
	XPropertyNotFound(String),

	#[error("Fail to create. Cause: {0}")]
	StoreFailToCreate(String),

	#[error(transparent)]
	SurrealError(#[from] surrealdb::Error),

	#[error(transparent)]
	IOError(#[from] std::io::Error),
}

// endregion:   ---- Error type enumerator

// region:      ---- Store

struct Store {
    ds: Datastore,
    ses: Session
}

impl Store {
    pub async fn new() -> Result<Self, Error> {
        let ds = Datastore::new("memory").await?;
		
		let ses = Session::for_db("test", "test");
		
		Ok(Store { ds, ses })
    }

    pub async fn get_list(&self) -> Result<Vec<Object>, Error> {
        let sql = "SELECT * FROM todo";

        let res = self.ds.execute(sql, &self.ses, None, true).await?;
        
		let first_res = res.into_iter().next().expect("Did not get a response");

		let array: Array = W(first_res.result?).try_into()?;

		array.into_iter().map(|value| W(value).try_into()).collect()
    }

    pub async fn get(&self, uid: &str) -> Result<Object, Error> {
        let sql = "SELECT * FROM todo WHERE id = $id";
        
		let vars: BTreeMap<String, Value> = BTreeMap::from([(
            "id".into(), thing(uid)?.into()
        )]);

        let res = self.ds.execute(sql, &self.ses, Some(vars), true).await?;
        
		let first_res = res.into_iter().next().expect("Did not get a response!");
        
		W(first_res.result?.first()).try_into()
    }

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
    
	pub async fn update(&self, tid: &str) -> Result<String, Error> {
		let sql = "UPDATE $th MERGE { body: 'An Updated message!', title: 'Updated!' } RETURN id";
		
		let vars: BTreeMap<String, Value> = BTreeMap::from([(
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
    
	pub async fn delete(&self, tid: &str) -> Result<String, Error> {
		let sql = "DELETE $th";

		let vars = BTreeMap::from([("th".into(), thing(tid)?.into())]);

		let ress = self.ds.execute(sql, &self.ses, Some(vars), false).await?;

		let first_res = ress.into_iter().next().expect("Did not get a response");

		first_res.result?;

		Ok(tid.to_string())
    }
}

// endregion:   ---- Store