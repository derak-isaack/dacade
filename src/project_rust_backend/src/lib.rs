use candid::{CandidType, Principal};
use ic_cdk::api::trap;
use ic_cdk::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::cell::RefCell;

thread_local! {
    static NEXT_ID: RefCell<u64> = RefCell::new(0);
    static FILES: RefCell<HashMap<u64, File>> = RefCell::new(HashMap::new());
}

#[derive(CandidType, Serialize, Deserialize, Clone)]
struct File {
    id: u64,
    data: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, CandidType)]
struct Event {
    id: String,
    name: String,
    date: String,
    total_tickets: u32,
    tickets_sold: u32,
}

#[derive(Serialize, Deserialize, Clone, CandidType)]
struct Ticket {
    event_id: String,
    buyer: Principal,
}

#[derive(Default, Serialize, Deserialize, CandidType)]
struct TicketingSystem {
    events: HashMap<String, Event>,
    tickets: Vec<Ticket>,
}


static mut TICKETING_SYSTEM: Option<TicketingSystem> = None;

#[ic_cdk::init]
fn init() {
    unsafe {
        TICKETING_SYSTEM = Some(TicketingSystem::default());
    }
}

#[ic_cdk::update]
fn create_event(id: String, name: String, date: String, total_tickets: u32) {
    let mut system = get_system_mut();
    if system.events.contains_key(&id) {
        trap("Event ID already exists!");
    }
    let event = Event {
        id: id.clone(),
        name,
        date,
        total_tickets,
        tickets_sold: 0,
    };
    system.events.insert(id, event);
}

#[ic_cdk::update]
fn buy_ticket(event_id: String) {
    let mut system = get_system_mut();
    let event = system
        .events
        .get_mut(&event_id)
        .expect("Event not found");

    if event.tickets_sold >= event.total_tickets {
        trap("No tickets available!");
    }

    let ticket = Ticket {
        event_id: event_id.clone(),
        buyer: ic_cdk::caller(),
    };

    system.tickets.push(ticket);
    event.tickets_sold += 1;
}

#[ic_cdk::query]
fn get_event(event_id: String) -> Option<Event> {
    let system = get_system();
    system.events.get(&event_id).cloned()
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

#[ic_cdk::query]
fn __export_candid_interface() -> String {
    ic_cdk::export_candid!(); 
    __export_service()
}


#[derive(CandidType, Serialize, Deserialize, Clone)]
struct Image {
    id: u64,
    data: Vec<Vec<u8>>,
}

#[ic_cdk::update]
fn upload_photo(file_data: Vec<u8>) -> u64 {
    NEXT_ID.with(|next_id| {
        let id = *next_id.borrow();
        *next_id.borrow_mut() += 1;

        FILES.with(|files| {
            files.borrow_mut().insert(
                id,
                File {
                    id,
                    data: file_data,
                }
            );
        });
        id
    })
}

#[ic_cdk::query]
fn get_file(id: u64) -> Option<File> {
    FILES.with(|files| {
        files.borrow().get(&id).cloned()
    })
}

ic_cdk::export_candid!();