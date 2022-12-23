use async_trait::async_trait;
use genbu_stores::{
    stores::{DataStore, Reset, Setup},
    users::{User, UserError, UserStore, UserUpdate},
    Uuid,
};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc};

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
    type StoreUser = User;
    type StoreError = UserError;

    async fn int_add(&mut self, user: &User) -> Result<(), Self::StoreError> {
        if let Ok(Some(u)) = self.get_by_email(&user.email).await {
            return Err(UserError::EmailAlreadyExists(u.email));
        }

        let mut users = self.users.lock();

        match users.insert(user.id, user.clone()) {
            None => Ok(()),
            Some(old_user) => {
                users.insert(user.id, old_user);
                Err(UserError::IDAlreadyExists(Some(user.id)))
            }
        }
    }

    async fn int_delete(&mut self, id: &Uuid) -> Result<Option<User>, Self::StoreError> {
        self.users
            .lock()
            .remove(id)
            .map_or_else(|| Ok(None), |user| Ok(Some(user)))
    }

    async fn int_get(&self, id: &Uuid) -> Result<Option<User>, Self::StoreError> {
        self.users
            .lock()
            .get(id)
            .map_or_else(|| Ok(None), |user| Ok(Some(user.clone())))
    }

    async fn int_get_all(&self) -> Result<Vec<User>, Self::StoreError> {
        Ok(self
            .users
            .lock()
            .iter()
            .map(|(_, val)| val.clone())
            .collect::<Vec<User>>())
    }

    async fn int_get_by_email(&self, email: &str) -> Result<Option<User>, Self::StoreError> {
        Ok(self
            .users
            .lock()
            .iter()
            .find(|(_, user)| user.email == email)
            .map(|(_, user)| user.clone()))
    }

    async fn update(&mut self, update: UserUpdate) -> Result<Option<User>, UserError> {
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
