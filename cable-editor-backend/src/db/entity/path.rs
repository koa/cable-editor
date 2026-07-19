use async_graphql::Enum;
use std::{fmt::Debug, marker::PhantomData};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Enum)]
pub enum DuctDirection {
    Forward,
    Backward,
}
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct DirectedDuct<SI: UnalignedDuct<S>, S: PartialEq> {
    pub direction: DuctDirection,
    pub duct: SI,
    phantom: PhantomData<S>,
}

impl<SI: UnalignedDuct<S>, S: PartialEq> DirectedDuct<SI, S> {
    pub fn schacht_a(&self) -> S {
        match self.direction {
            DuctDirection::Forward => self.duct.schacht_a(),
            DuctDirection::Backward => self.duct.schacht_z(),
        }
    }
    pub fn schacht_z(&self) -> S {
        match self.direction {
            DuctDirection::Forward => self.duct.schacht_z(),
            DuctDirection::Backward => self.duct.schacht_a(),
        }
    }
}

pub trait UnalignedDuct<S: PartialEq> {
    fn schacht_a(&self) -> S;
    fn schacht_z(&self) -> S;
}

pub struct AlignedDuctIterator<I: Iterator, S>
where
    I::Item: UnalignedDuct<S>,
    S: std::cmp::PartialEq,
{
    source: I,
    last_schacht: Option<S>,
    temp_entry: Option<I::Item>,
}

impl<I: Iterator, S> AlignedDuctIterator<I, S>
where
    I::Item: UnalignedDuct<S>,
    S: std::cmp::PartialEq,
{
    fn forward_result(
        duct: <I as Iterator>::Item,
    ) -> Option<
        Result<
            DirectedDuct<<I as Iterator>::Item, S>,
            DuctAlignmentError<<I as Iterator>::Item, S>,
        >,
    > {
        Some(Ok(DirectedDuct {
            direction: DuctDirection::Forward,
            duct,
            phantom: Default::default(),
        }))
    }
    fn backward_result(
        duct: <I as Iterator>::Item,
    ) -> Option<
        Result<
            DirectedDuct<<I as Iterator>::Item, S>,
            DuctAlignmentError<<I as Iterator>::Item, S>,
        >,
    > {
        Some(Ok(DirectedDuct {
            direction: DuctDirection::Backward,
            duct,
            phantom: Default::default(),
        }))
    }
}

#[derive(Debug)]
pub enum DuctAlignmentError<Item: UnalignedDuct<S>, S: PartialEq> {
    NoConnectionFoundOnPair { first: Item, second: Item },
    NoConnectionFoundForSchacht { last_schacht: S, duct: Item },
}

impl<SI: Iterator, S: PartialEq> Iterator for AlignedDuctIterator<SI, S>
where
    SI::Item: UnalignedDuct<S>,
{
    type Item = Result<DirectedDuct<SI::Item, S>, DuctAlignmentError<SI::Item, S>>;

    fn next(&mut self) -> Option<Self::Item> {
        let duct = self.temp_entry.take().or_else(|| self.source.next())?;
        let entry_schacht_a = duct.schacht_a();
        let entry_schacht_z = duct.schacht_z();
        if let Some(last_schacht) = self.last_schacht.take() {
            if entry_schacht_a == last_schacht {
                self.last_schacht = Some(entry_schacht_z);
                Self::forward_result(duct)
            } else if entry_schacht_z == last_schacht {
                self.last_schacht = Some(entry_schacht_a);
                Self::backward_result(duct)
            } else {
                Some(Err(DuctAlignmentError::NoConnectionFoundForSchacht {
                    last_schacht,
                    duct,
                }))
            }
        } else {
            if let Some(second_entry) = self.source.next() {
                if entry_schacht_a == second_entry.schacht_a()
                    || entry_schacht_a == second_entry.schacht_z()
                {
                    self.temp_entry = Some(second_entry);
                    self.last_schacht = Some(entry_schacht_a);
                    Self::backward_result(duct)
                } else if entry_schacht_z == second_entry.schacht_a()
                    || entry_schacht_z == second_entry.schacht_z()
                {
                    self.temp_entry = Some(second_entry);
                    self.last_schacht = Some(entry_schacht_z);
                    Self::forward_result(duct)
                } else {
                    Some(Err(DuctAlignmentError::NoConnectionFoundOnPair {
                        first: duct,
                        second: second_entry,
                    }))
                }
            } else {
                Self::forward_result(duct)
            }
        }
    }
}

pub fn align_ducts<I, Sch>(iter: I) -> AlignedDuctIterator<I, Sch>
where
    I: Iterator,
    I::Item: UnalignedDuct<Sch>,
    Sch: std::cmp::PartialEq,
{
    AlignedDuctIterator {
        source: iter,
        last_schacht: None,
        temp_entry: None,
    }
}
