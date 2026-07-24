use patternfly_yew::prelude::{
    ExpansionState, StateModel, StateModelIter, TableDataModel, TableModel, UseTableData,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

pub struct ListModel<C, M>
where
    C: Clone + Eq + 'static,
    M: PartialEq + Clone + TableDataModel<C> + 'static,
    M::Key: Hash,
{
    model: Rc<StateModel<C, M>>,
}

impl<C, M> ListModel<C, M>
where
    C: Clone + Eq + 'static,
    M: PartialEq + Clone + TableDataModel<C> + 'static,
    M::Key: Hash,
{
    pub fn new(data: M, state: Rc<RefCell<HashMap<M::Key, ExpansionState<C>>>>) -> ListModel<C, M> {
        ListModel {
            model: Rc::new(StateModel::new(data, state)),
        }
    }
}

impl<C, M> TableModel<C> for ListModel<C, M>
where
    C: Clone + Eq + 'static,
    M: PartialEq + Clone + TableDataModel<C> + 'static,
    M::Key: Hash,
{
    type Iterator<'i> = StateModelIter<'i, M::Key, M::Item, C>;
    type Item = M::Item;
    type Key = M::Key;

    fn len(&self) -> usize {
        self.model.len()
    }

    fn is_empty(&self) -> bool {
        self.model.is_empty()
    }

    fn iter(&self) -> Self::Iterator<'_> {
        self.model.iter()
    }
}

impl<C, M> Clone for ListModel<C, M>
where
    C: Clone + Eq + 'static,
    M: PartialEq + Clone + TableDataModel<C> + 'static,
    M::Key: Hash,
{
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
        }
    }
}

impl<C, M> PartialEq for ListModel<C, M>
where
    C: Clone + Eq + 'static,
    M: PartialEq + Clone + TableDataModel<C> + 'static,
    M::Key: Hash,
{
    fn eq(&self, other: &Self) -> bool {
        self.model == other.model
    }
}
