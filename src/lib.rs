//! mem_table is an in-memory table of data, modeled after [Pandas](https://pandas.pydata.org/) for Python.
#[macro_use]
extern crate log;

use std::io::{Error as IOError, Read};
use std::path::Path;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter, Error as FmtError};
use std::hash::{Hash, Hasher};
use std::cell::Ref;
use std::iter::FusedIterator;
use std::ops::Index;

use rayon::prelude::*;
use csv::{Reader, Writer};

mod value;
mod row;
mod table_error;
mod row_table;

// expose some of the underlying structures from other files
//pub use crate::row_table::RowTable;
pub use crate::value::Value;
use crate::table_error::TableError;
use crate::row::{Row, RowIntoIter, OwnedRow};

// Playground: https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=98ca951a70269d44cb48230359857f60

/// The main interface into the mem_table library
pub trait Table<'a>: TableOperations<'a> {
    /// Create a blank RowTable
    fn new(columns :&[&str]) -> Self;

    /// Read in a CSV file, and construct a RowTable
    fn from_csv<P: AsRef<Path>>(path: P) -> Result<Self, IOError> where Self: Sized;

    fn append<'b>(&mut self, table :impl TableOperations<'b>) -> Result<(), TableError> {
        // make sure the columns are the same
        if !self.columns().iter().zip(table.columns().iter()).all(|(a, b)| a == b) {
            let err_str = format!("Columns don't match between tables: {:?} != {:?}", self.columns(), table.columns());
            return Err(TableError::new(err_str.as_str()));
        }

        for row in table.into_iter() {
//            self.append_row(row);
        }

        Ok( () )
    }

    fn append_row(&mut self, row :Row) -> Result<(), TableError>;

    /// Adds a column with `column_name` to the end of the table filling in all rows with `value`.
    /// This method works in parallel and is therefore usually faster than `add_column_with`
    fn add_column(&mut self, column_name :&str, value :&Value) -> Result<(), TableError> {
        self.add_column_with(column_name, || value.clone())
    }

    /// Adds a column with `column_name` to the end of the table using `f` to generate the values for each row.
    /// This method works a row-at-a-time and therefore can be slower than `add_column`.
    fn add_column_with<F: FnMut() -> Value>(&mut self, column_name :&str, f :F) -> Result<(), TableError>;
}

/// Operations that can be performed on `Table`s or `TableSlice`s.
pub trait TableOperations<'a> {
    type TableSliceType: TableSlice<'a>;
//    type RowIteratorType: Iterator<Item=&'a Row>;

    fn iter(&self) -> Box<dyn Iterator<Item=Row>>;
    fn into_iter(self) -> Box<dyn Iterator<Item=OwnedRow>>;
//    fn row_mut_iter(&mut self) -> RowMutIter;

    fn columns(&self) -> &Vec<String>;

    fn column_position(&self, column :&str) -> Result<usize, TableError> {
        if let Some(pos) = self.columns().iter().position(|c| c == column) {
            Ok(pos)
        } else {
            Err(TableError::new(format!("Column not found: {}", column).as_str()))
        }
    }

    #[inline]
    fn len(&'a self) -> usize {
        self.iter().count()
    }

    #[inline]
    fn width(&self) -> usize {
        self.columns().len()
    }

    /// Write a table out to a CSV file
    fn to_csv(&'a self, csv_path :&Path) -> Result<(), TableError> {
        let mut csv = Writer::from_path(csv_path).map_err(|e| TableError::new(e.to_string().as_str()))?;

        // write out the headers first
        csv.write_record(self.columns());

        // go through each row, writing the records converted to Strings
        for row in self.iter() {
            csv.write_record(row.iter().map(|f| String::from(f)));
        }

        Ok( () )
    }

    fn group_by(&'a self, column :&str) -> Result<HashMap<&Value, Self::TableSliceType>, TableError>;

    fn unique(&'a self, column :&str) -> Result<HashSet<&'a Value>, TableError>  {
        // get the position in the row we're concerned with
        let pos = self.column_position(column)?;

        // insert the values into the HashSet
        // TODO: use Rayon to make this go in parallel
        Ok(self.iter().map(|row| row.at(pos).unwrap()).collect::<HashSet<_>>())
    }

    /// Returns a `TableSlice` with all rows that where `value` matches in the `column`.
    fn find(&'a self, column :&str, value :&Value) -> Result<Self::TableSliceType, TableError> {
        // get the position in the underlying table
        let pos = self.column_position(column)?;

        self.find_by(|row| row[pos] == *value)
    }

    fn find_by<P: FnMut(&Vec<Value>) -> bool>(&'a self, predicate :P) -> Result<Self::TableSliceType, TableError>;
}

/// A `TableSlice` is a view into a `Table`.
pub trait TableSlice<'a>: TableOperations<'a> {
    fn column_position(&self, column :&str) -> Result<usize, TableError> {
        if self.columns().iter().find(|c| c.as_str() == column).is_none() {
            let err_str = format!("Could not find column in slice: {}", column);
            return Err(TableError::new(err_str.as_str()));
        }

        TableOperations::column_position(self, column)
    }
}


// these are for tests
#[cfg(test)] extern crate simple_logger;
#[cfg(test)] extern crate rand;
#[cfg(test)] use std::sync::{Once};
#[cfg(test)] static LOGGER_INIT: Once = Once::new();

