use candid::{CandidType, Principal, Nat, Decode, Encode};
use ic_cdk::api::trap;
use ic_cdk::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{borrow::Cow, cell::RefCell};
use ic_cdk::api::call::CallResult;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};


type Memory1 = VirtualMemory<DefaultMemoryImpl>;

#[derive(CandidType, Serialize, Deserialize, Clone)]
struct File {
    id: u64,
    data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, CandidType)]
struct Event {
    id : u64, 
    name: String,
    date: String,
    total_tickets: u32,
    tickets_sold: u32,
}

#[derive(Serialize, Deserialize, Clone, CandidType)]
struct Ticket {
    event_id: String,
    buyer: Principal,
    purchase_date: String,
    ticket_number: u32,
}

#[derive(Default, Serialize, Deserialize, CandidType)]
struct TicketingSystem {
    events: HashMap<String, Event>,
    tickets: Vec<Ticket>,
}

impl Storable for Event {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Event {
    const MAX_SIZE: u32 = 1024; // Maximum size in bytes for a single note
    const IS_FIXED_SIZE: bool = false;
}

impl Storable for File {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for File {
    const MAX_SIZE: u32 = 2048; 
    const IS_FIXED_SIZE: bool = false;
}


thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static EVENTS: RefCell<StableBTreeMap<u64, Event, Memory1>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0)))
        )
    );

    static EVENTPHOTOS: RefCell<StableBTreeMap<u64, File, Memory1>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(5)))
        )
    );

    static COUNTER: RefCell<Cell<u64, Memory1>> = RefCell::new(
        Cell::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
            0
        ).unwrap()
    );
}

static mut TICKETING_SYSTEM: Option<TicketingSystem> = None;

#[ic_cdk::init]
fn init() {
    unsafe {
        TICKETING_SYSTEM = Some(TicketingSystem::default());
    }
}

#[ic_cdk::update]
fn create_event(name: String, date: String, total_tickets: u32) -> Event {
    let id = COUNTER.with(|counter| {
        let current_value = *counter.borrow().get();
        counter.borrow_mut().set(current_value + 1).unwrap();
        current_value
    });
    let event = Event {
        id,
        name,
        date,
        total_tickets,
        tickets_sold: 0,
    };
    EVENTS.with(|events| {
        events.borrow_mut().insert(id, event.clone())
    });

    event 
}

#[derive(CandidType)]
struct Account {
    owner: Principal,
    subaccount: Option<[u8; 32]>,
}

#[derive(CandidType)]
struct TransferArgs {
    memo: Nat,
    amount: Nat,
    fee: Nat,
    from_subaccount: Option<[u8; 32]>,
    to: Account,
    created_at_time: Option<u64>,
}

#[ic_cdk::update]
async fn buy_ticket(event_id: String) -> Result<(), String> {
    let caller = ic_cdk::caller();
    let mut system = get_system_mut();
    
    let event = system
        .events
        .get_mut(&event_id)
        .ok_or_else(|| "Event not found".to_string())?;
    
    if event.tickets_sold >= event.total_tickets {
        return Err("Event is sold out!".to_string());
    }
    
    if system.tickets.iter().any(|ticket| ticket.event_id == event_id && ticket.buyer == caller) {
        return Err("You already have a ticket for this event".to_string());
    }
    
    let amount = Nat::from(100_000_000u64); 
    let fee = Nat::from(10_000u64); 

    let transfer_args = TransferArgs {
        memo: Nat::from(0u64),
        amount,
        fee,
        from_subaccount: None,
        to: Account {
            owner: ic_cdk::id(),
            subaccount: None,
        },
        created_at_time: Some(ic_cdk::api::time()),
    };

    let ledger_canister_id = Principal::from_text("ryjl3-tyaaa-aaaaa-aaaba-cai")
        .expect("Invalid ledger canister ID");
    
    let result: CallResult<(Result<Nat, String>,)> = 
        ic_cdk::call(ledger_canister_id, "transfer", (transfer_args,)).await;

    match result {
        Ok((Ok(_block_index),)) => {
            let ticket = Ticket {
                event_id: event_id.clone(),
                buyer: caller,
                purchase_date: ic_cdk::api::time().to_string(),
                ticket_number: event.tickets_sold + 1,
            };

            system.tickets.push(ticket);
            event.tickets_sold += 1;
            Ok(())
        },
        Ok((Err(transfer_error),)) => {
            Err(format!("Payment failed: {}", transfer_error))
        },
        Err((code, msg)) => {
            Err(format!("Transaction failed: {:?} - {}", code, msg))
        },
    }
}

#[ic_cdk::query]
fn get_event(id: u64) -> Option<Event> {
    EVENTS.with(|events| events.borrow().get(&id))
}

#[ic_cdk::query]
fn get_tickets() -> Vec<Ticket> {
    let system = get_system();
    system.tickets.clone()
}

fn get_system() -> &'static TicketingSystem {
    unsafe {
        TICKETING_SYSTEM
            .as_ref()
            .expect("TicketingSystem not initialized")
    }
}

fn get_system_mut() -> &'static mut TicketingSystem {
    unsafe {
        TICKETING_SYSTEM
            .as_mut()
            .expect("TicketingSystem not initialized")
    }
}

#[derive(CandidType, Serialize, Deserialize, Clone)]
struct Image {
    id: u64,
    data: Vec<Vec<u8>>,
}

#[ic_cdk::update]
fn upload_photo(file_data: Vec<u8>) -> File {
    let id = COUNTER.with(|counter| {
        let current_value = *counter.borrow().get();
        counter.borrow_mut().set(current_value + 1).unwrap();
        current_value
    });

    let file = File {
        id,
        data: file_data,
    };
    
    EVENTPHOTOS.with(|files| {
        files.borrow_mut().insert(id, file.clone())
    });

    file
}

#[ic_cdk::query]
fn get_file(id: u64) -> Option<File> {
    EVENTPHOTOS.with(|files| files.borrow().get(&id))
}

ic_cdk::export_candid!();