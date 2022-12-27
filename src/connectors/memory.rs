use async_trait::async_trait;
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc};

use crate::stores::{
    users::{SResult, User, UserError, UserStore, UserUpdate},
    DataStore, Reset, Setup, Uuid,
};

#[derive(Clone, Default)]
pub struct MemStore {
    users: Arc<Mutex<HashMap<Uuid, User>>>,
}

impl MemStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl UserStore for MemStore {
    async fn add(&mut self, user: &User) -> SResult<()> {
        if let Ok(Some(u)) = self.get_by_email(&user.email).await {
            return Err(UserError::EmailAlreadyExists(u.email));
        }

        let mut users = self.users.lock();

        users.insert(user.id, user.clone()).map_or_else(
            || Ok(()),
            |old_user| {
                users.insert(user.id, old_user);
                Err(UserError::IDAlreadyExists(Some(user.id)))
            },
        )
    }

    async fn delete(&mut self, id: &Uuid) -> SResult<Option<User>> {
        self.users
            .lock()
            .remove(id)
            .map_or_else(|| Ok(None), |user| Ok(Some(user)))
    }

    async fn get(&self, id: &Uuid) -> SResult<Option<User>> {
        self.users
            .lock()
            .get(id)
            .map_or_else(|| Ok(None), |user| Ok(Some(user.clone())))
    }

    async fn get_all(&self) -> SResult<Vec<User>> {
        Ok(self
            .users
            .lock()
            .iter()
            .map(|(_, val)| val.clone())
            .collect::<Vec<User>>())
    }

    async fn get_by_email(&self, email: &str) -> SResult<Option<User>> {
        Ok(self
            .users
            .lock()
            .iter()
            .find(|(_, user)| user.email == email)
            .map(|(_, user)| user.clone()))
    }

    async fn update(&mut self, update: UserUpdate) -> SResult<Option<User>> {
        let user = self.get(&update.id).await?;
        let Some(mut user) = user else {
            return Ok(None)
        };
        if let Some(update_name) = update.name {
            user.name = update_name;
        }
        if let Some(update_avatar) = update.avatar {
            user.avatar = Some(update_avatar);
        }
        Ok(self.users.lock().insert(user.id, user))
    }
}

#[async_trait]
impl DataStore for MemStore {
    async fn new(_: String) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self::new())
    }
}

#[async_trait]
impl Reset for MemStore {
    #[cfg(debug_assertions)]
    async fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[async_trait]
impl Setup for MemStore {
    async fn setup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}
