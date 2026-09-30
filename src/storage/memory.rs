//! 테스트용 in-memory 저장소.

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::core::error::PhError;
use crate::core::model::{Prompt, PromptId, Scope};
use crate::core::storage::{Listing, Storage};

/// 메모리에만 저장하는 `Storage`. 스레드 간 공유는 지원하지 않는다.
#[derive(Debug)]
pub struct MemoryStorage {
    scope: Scope,
    items: RefCell<BTreeMap<String, Prompt>>,
}

impl MemoryStorage {
    /// 빈 저장소를 만든다.
    pub fn new(scope: Scope) -> Self {
        Self {
            scope,
            items: RefCell::new(BTreeMap::new()),
        }
    }
}

impl Storage for MemoryStorage {
    fn scope(&self) -> Scope {
        self.scope
    }

    fn location(&self) -> String {
        "memory".to_string()
    }

    fn list(&self) -> Result<Listing, PhError> {
        Ok(Listing {
            prompts: self.items.borrow().values().cloned().collect(),
            skipped: Vec::new(),
        })
    }

    fn get(&self, id: &PromptId) -> Result<Option<Prompt>, PhError> {
        Ok(self.items.borrow().get(id.as_str()).cloned())
    }

    fn put(&self, prompt: &Prompt) -> Result<(), PhError> {
        let mut p = prompt.clone();
        p.scope = self.scope;
        self.items.borrow_mut().insert(p.id.as_str().to_string(), p);
        Ok(())
    }

    fn delete(&self, id: &PromptId) -> Result<(), PhError> {
        match self.items.borrow_mut().remove(id.as_str()) {
            Some(_) => Ok(()),
            None => Err(PhError::NotFound {
                id: id.as_str().to_string(),
            }),
        }
    }
}
