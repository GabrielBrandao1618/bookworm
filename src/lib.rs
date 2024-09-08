#[cfg(test)]
pub mod tests;

use std::{
    fmt::Debug,
    io::{Read, Seek, Write},
};

use error::BookwormResult;
use pager::{Pager, PagerIterator, RawPagerIterator};
use serde::{de::DeserializeOwned, ser::Serialize};

pub mod error;
mod pager;

pub struct Bookworm<'a, S: Read + Write + Seek> {
    pager: Pager<'a, S>,
    swap: Pager<'a, S>,
    page_size: usize,
}
impl<'a, S: Read + Write + Seek> Bookworm<'a, S> {
    pub fn new(page_size: usize, data_source: &'a mut S, swap: &'a mut S) -> Self {
        Self {
            page_size,
            pager: Pager::new(page_size, data_source),
            swap: Pager::new(page_size, swap),
        }
    }
    pub fn get_page<T: DeserializeOwned + Debug>(&mut self, page: usize) -> BookwormResult<T> {
        self.pager.get_page(page)
    }
    pub fn get_raw_page(&mut self, page: usize) -> BookwormResult<Vec<u8>> {
        self.pager.get_raw_page(page)
    }
    pub fn write_raw_page(&mut self, page: usize, data: &[u8]) -> BookwormResult<()> {
        self.pager.write_raw_page(page, data)
    }
    pub fn write_page<T: Serialize>(&mut self, page: usize, data: &T) -> BookwormResult<()> {
        self.pager.write_page(page, data)
    }
    pub fn get_raw_iterator(self) -> RawPageIterator<'a, S> {
        self.into()
    }
    pub fn get_iterator<T: DeserializeOwned>(self) -> PageIterator<'a, S, T> {
        self.into()
    }
    pub fn push<T: Serialize>(&mut self, data: &T) -> BookwormResult<()> {
        self.pager.push(data)
    }
    pub fn pop(&mut self) -> BookwormResult<()> {
        self.pager.pop()
    }
    pub fn delete(&mut self, page: usize) -> BookwormResult<()> {
        Ok(())
    }
}

pub struct RawPageIterator<'a, S: Read + Write + Seek> {
    pager_iterator: RawPagerIterator<'a, S>,
}

impl<'a, S: Read + Write + Seek> Into<RawPageIterator<'a, S>> for Bookworm<'a, S> {
    fn into(self) -> RawPageIterator<'a, S> {
        RawPageIterator {
            pager_iterator: self.pager.into_raw_iterator(0),
        }
    }
}

impl<S: Read + Write + Seek> Iterator for RawPageIterator<'_, S> {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        self.pager_iterator.next()
    }
}

pub struct PageIterator<'a, S: Read + Write + Seek, T: DeserializeOwned> {
    pager_iterator: PagerIterator<'a, S, T>,
    _marker: std::marker::PhantomData<T>,
}

impl<S, T> Iterator for PageIterator<'_, S, T>
where
    S: Read + Write + Seek,
    T: DeserializeOwned,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.pager_iterator.next()
    }
}

impl<'a, S: Read + Write + Seek, T: DeserializeOwned> Into<PageIterator<'a, S, T>>
    for Bookworm<'a, S>
{
    fn into(self) -> PageIterator<'a, S, T> {
        let _ = self.pager.data_source.rewind();
        PageIterator {
            pager_iterator: self.pager.into_iterator(0),
            _marker: Default::default(),
        }
    }
}
