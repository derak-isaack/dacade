use candid::{CandidType, Principal,Decode, Encode};
// use ic_cdk::api::{caller, call::call};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{borrow::Cow, cell::RefCell};
// use ic_cdk::api::call::CallResult;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use ic_ledger_types::{AccountIdentifier, BlockIndex, Memo, TransferArgs, Tokens, DEFAULT_SUBACCOUNT, DEFAULT_FEE, MAINNET_LEDGER_CANISTER_ID, transfer};

type Memory1 = VirtualMemory<DefaultMemoryImpl>;

#[derive(CandidType, Serialize, Deserialize, Clone)]
struct File {
    id: u64,
    data: Vec<u8>,
}


#[derive(Serialize, Deserialize, Clone, CandidType)]
struct Event {
    id : u64, 
    price: Tokens,
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
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

impl Storable for Ticket {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Ticket {
    const MAX_SIZE: u32 = 1024;
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

    static TICKETS: RefCell<StableBTreeMap<u64, Ticket, Memory1>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(2)))
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
fn create_event(name: String, date: String, total_tickets: u32, price: Tokens) -> Event {
    let id = COUNTER.with(|counter| {
        let current_value = *counter.borrow().get();
        counter.borrow_mut().set(current_value + 1).unwrap();
        current_value
    });
    let event = Event {
        id,
        name,
        price,
        date,
        total_tickets,
        tickets_sold: 0,
    };
    EVENTS.with(|events| {
        events.borrow_mut().insert(id, event.clone())
    });

    event 
}

async fn transfer_to_caller(amount: Tokens) -> Result<BlockIndex, String> {
    transfer(
        MAINNET_LEDGER_CANISTER_ID,
        TransferArgs {
            memo: Memo(0),
            amount,
            fee: DEFAULT_FEE,
            from_subaccount: None,
            to: AccountIdentifier::new(&ic_cdk::caller(), &DEFAULT_SUBACCOUNT),
            created_at_time: None,
        }
    )
    .await
    .map_err(|e| format!("Call to ledger failed: {:?}", e))? 
    .map_err(|err| format!("Transfer failed: {:?}", err)) 
}

#[ic_cdk::update]
async fn buy_ticket(price: Tokens) -> Result<String, String> {
    let caller = ic_cdk::caller();

    let mut selected_event = None;
    let mut selected_event_id = 0;

    EVENTS.with(|events| {
        let events = events.borrow();
        for (id, event) in events.iter() {
            if event.price == price && event.tickets_sold < event.total_tickets {
                selected_event = Some(event);
                selected_event_id = id;
                break;
            }
        }
    });

    let event = selected_event.ok_or_else(|| "No event found with that price.".to_string())?;

    let already_booked = TICKETS.with(|tickets| {
        let tickets = tickets.borrow();
        tickets.iter().any(|(_, ticket)| ticket.event_id == selected_event_id.to_string() && ticket.buyer == caller)
    });

    if already_booked {
        return Err("You already have a ticket for this event.".to_string());
    }

    transfer_to_caller(price)
        .await
        .map_err(|err| format!("Payment failed: {}", err))?;

    let ticket_number = event.tickets_sold + 1;
    let ticket = Ticket {
        event_id: selected_event_id.to_string(),
        buyer: caller,
        purchase_date: ic_cdk::api::time().to_string(),
        ticket_number,
    };

    TICKETS.with(|tickets| {
        tickets.borrow_mut().insert(ticket_number as u64, ticket.clone())
    });
    EVENTS.with(|events| {
        let mut events = events.borrow_mut();
        if let Some(mut event) = events.get(&selected_event_id) {
            let mut updated_event = event.clone();
            updated_event.tickets_sold += 1;
            events.insert(selected_event_id, updated_event);
        }
    });

    Ok(format!(
        "Successfully booked a ticket for event '{}' at price {:?}. Ticket number: {}.",
        event.name, price, ticket_number
    ))
}



#[ic_cdk::query]
fn get_event(id: u64) -> Option<Event> {
    EVENTS.with(|events| events.borrow().get(&id))
}

#[ic_cdk::query]
fn get_tickets() -> Vec<Ticket> {
    TICKETS.with(|tickets| {
        tickets.borrow().iter().map(|(_, ticket)| ticket.clone()).collect()
    })
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