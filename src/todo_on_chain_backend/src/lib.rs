use candid::{CandidType, Decode, Deserialize, Encode, Principal};
use ic_cdk::{caller, query, update, api::time};
use ic_stable_structures::{
    memory_manager::{MemoryId, MemoryManager, VirtualMemory},
    BoundedStorable, DefaultMemoryImpl, StableBTreeMap, StableCell, Storable,
};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(Deserialize, CandidType)]
struct TodoItem {
    caller: Principal,
    id: u64,
    assignee: String,
    description: String,
    duration: u64,
    is_active: bool,
    updated_at: u64,
    created_at: u64,
}

#[derive(Deserialize, CandidType)]
struct TodoItemView {
    assignee: String,
    description: String,
    duration: u64,
    is_active: bool,
}

#[derive(CandidType)]
enum Messages {
    Success,
    NotAuthorized,
    NoSuchItem,
}

impl Storable for TodoItem {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for TodoItem {
    const MAX_SIZE: u32 = 500;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
static MEMORY_MANAGER : RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

static TODO_MAP: RefCell<StableBTreeMap<u64, TodoItem, Memory>> = RefCell::new(StableBTreeMap::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0)))));

static ID_COUNTER: RefCell<StableCell<u64, Memory>> = RefCell::new(StableCell::init(
    MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
    u64::default()).unwrap());
}

fn get_and_inc_current_id() -> u64 {
    let mut id_tmp = 0;
    ID_COUNTER.with(|counter| {
        id_tmp = *(counter.borrow()).get();
        counter.borrow_mut().set(id_tmp + 1).unwrap();
    });
    return id_tmp;
}

#[query(name = "fetch_all")]
fn get_todo_list_vector() -> Option<Vec<TodoItem>> {
    let mut vector: Vec<TodoItem> = Vec::new();

    TODO_MAP.with(|p| {
        for (_, v) in p.borrow_mut().iter() {
            vector.push(v);
        }
    });
    Some(vector)
}

#[query(name = "get")]
fn get_todo(key: u64) -> Option<TodoItem> {
    TODO_MAP.with(|p| p.borrow_mut().get(&key))
}

#[update(name = "create")]
fn insert_new_todo(value: TodoItemView) -> Option<TodoItem> {
    let id_tmp = get_and_inc_current_id();

    let new_todo: TodoItem = TodoItem {
        caller: caller(),
        id: id_tmp,
        assignee: value.assignee,
        description: value.description,
        duration: value.duration,
        is_active: value.is_active,
        updated_at: time(),
        created_at: time()
    };

    TODO_MAP.with(|p| {
        let mut borrowed_map = p.borrow_mut();
        if borrowed_map.contains_key(&id_tmp) {
            return None;
        }
        borrowed_map.insert(id_tmp, new_todo)
    })
}

#[update(name = "update")]
fn update_todo(key: u64, value: TodoItemView) -> Result<Messages, Messages> {
    let mut is_authorized: bool = true;
    let mut ret_todo: Option<TodoItem> = None;

    TODO_MAP.with(|todos| {
        for (k, mut v) in todos.borrow_mut().iter() {
            if k == key {
                if v.caller != caller() {
                    is_authorized = false;
                }
                v.assignee = value.assignee;
                v.description = value.description;
                v.duration = value.duration;
                v.is_active = value.is_active;
                v.updated_at = time();
                ret_todo = Some(v);
                break;
            }
        }
    });

    if !is_authorized {
        return Err(Messages::NotAuthorized);
    }

    match ret_todo {
        Some(_) => {
            TODO_MAP.with(|todo| todo.borrow_mut().insert(key, ret_todo.unwrap()));
            Ok(Messages::Success)
        }
        None => Err(Messages::NoSuchItem),
    }
}

#[update(name = "delete")]
fn delete_todo(key: u64) -> Result<Messages, Messages> {
    let mut found_todo: Option<TodoItem> = None;

    TODO_MAP.with(|items| {
        found_todo = items.borrow_mut().get(&key);
    });

    match found_todo {
        Some(ft) => {
            if ft.caller != caller() {
                return Err(Messages::NotAuthorized);
            }
            TODO_MAP.with(|items| {
                items.borrow_mut().remove(&key);
            });
            Ok(Messages::Success)
        }
        None => Err(Messages::NoSuchItem),
    }
}
