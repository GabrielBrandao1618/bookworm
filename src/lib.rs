#[cfg(test)]
pub mod tests;

use std::{
    cell::RefCell,
    fmt::Debug,
    io::{Read, Seek, Write},
    rc::Rc,
};

use error::BookwormResult;
use pager::{Pager, PagerIterator, RawPagerIterator};
use serde::{de::DeserializeOwned, ser::Serialize};

pub mod error;
mod pager;

pub struct Bookworm<S: Read + Write + Seek> {
    pager: Pager<S>,
    swap: Pager<S>,
    page_size: usize,
}
impl<'a, S: Read + Write + Seek> Bookworm<S> {
    pub fn new(page_size: usize, data_source: Rc<RefCell<S>>, swap: Rc<RefCell<S>>) -> Self {
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
    pub fn into_raw_iter(self) -> RawPageIterator<S> {
        self.into()
    }
    pub fn into_iter<T: DeserializeOwned>(self) -> PageIterator<S, T> {
        self.into()
    }
    pub fn push<T: Serialize>(&mut self, data: &T) -> BookwormResult<()> {
        self.pager.push(data)
    }
    pub fn pop(&mut self) -> BookwormResult<()> {
        self.pager.pop()
    }
    pub fn delete(&mut self, page: usize) -> BookwormResult<()> {
        let remaining_content_iter = self.pager.raw_iter(page + 1);
        for data in remaining_content_iter {
            self.swap.push_raw(&data)?;
        }
        let swap_iter = self.swap.raw_iter(0);
        for (i, data) in swap_iter.enumerate() {
            self.pager.write_raw_page(i + page, &data)?;
        }
        self.pager.pages_count -= 1;
        self.swap.clear();
        Ok(())
    }
}

pub struct RawPageIterator<S: Read + Write + Seek> {
    pager_iterator: RawPagerIterator<S>,
}

impl<'a, S: Read + Write + Seek> Into<RawPageIterator<S>> for Bookworm<S> {
    fn into(self) -> RawPageIterator<S> {
        RawPageIterator {
            pager_iterator: self.pager.into_raw_iterator(0),
        }
    }
}

impl<S: Read + Write + Seek> Iterator for RawPageIterator<S> {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        self.pager_iterator.next()
    }
}

pub struct PageIterator<S: Read + Write + Seek, T: DeserializeOwned> {
    pager_iterator: PagerIterator<S, T>,
    _marker: std::marker::PhantomData<T>,
}

impl<S, T> Iterator for PageIterator<S, T>
where
    S: Read + Write + Seek,
    T: DeserializeOwned,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.pager_iterator.next()
    }
}

impl<'a, S: Read + Write + Seek, T: DeserializeOwned> Into<PageIterator<S, T>> for Bookworm<S> {
    fn into(self) -> PageIterator<S, T> {
        let mut data_source = self.pager.data_source.borrow_mut();
        let _ = data_source.rewind();
        drop(data_source);
        PageIterator {
            pager_iterator: self.pager.into_iterator(0),
            _marker: Default::default(),
        }
    }
}
