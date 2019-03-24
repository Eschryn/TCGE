use super::lens;

/// A event that can be sent trough the router towards various destinations.
/// At the moment the only possible destination is a Lens.
pub trait Event: mopa::Any {
	///	If an event is passive, it can be fired at its destination
	///	regardless of what state the lens is in.
	fn is_passive(&self) -> bool;
}

// This is 100% necessary until `std::` provides Any for object-traits.
mopafy!(Event);

pub enum EventPhase {
	/// The event is being wrapped in a `EventWrapper`.
	Creation,
	
	/// The event is flowing towards its destination.
	Propagation,
	
	/// The event is being evaluated by its destination.
	Action,
	
	/// The event is flowing back towards its source.
	Bubbling
}

/// Wraps an event as it is processed by the [Router].
pub struct EventWrapper<'a> {
	#[allow(dead_code)]
	/// The event being processed.
	pub event: &'a mut Event,
	
	// --- State for the event
	phase: EventPhase,
	
	/// Can the event flow towards its destination?
	can_propagate: bool,
	
	/// Can the event be evaluated by its destination?
	can_default: bool,
	
	/// Can the event flow back towards its source?
	can_bubble: bool,
}

impl<'a> EventWrapper<'a> {
	/// Prevents the event from being evaluated by its destination.
	pub fn prevent_default(&mut self) {
		self.can_default = false;
	}
	
	/// Stops the flow of the event toward its destination.
	pub fn stop_propagation(&mut self) {
		self.can_propagate = false;
	}
	
	/// Stops the flow of the event back towards its source.
	pub fn stop_bubbling(&mut self) {
		self.can_bubble = false;
	}
}

/// Implementation details for event handling.
impl super::Router {
	/// Fires a single `Event` at a single `Lens`.
	pub fn fire_event_at_lens(&mut self, target: &str, event: &mut Event) {
		let lens_id = self.lenses.lenses.iter().position(|lens| { lens.name == target });
		let lens_id = match lens_id {
			Some(x) => x,
			None => return
		};
		
		self.fire_event_at_lens_id(lens_id, event);
	}
	
	/// Actual implementation for `fire_event_at_lens`.
	pub fn fire_event_at_lens_id(&mut self, lens_id: usize, event: &mut Event) {
		let lens = self.lenses.lenses.get_mut(lens_id);
		
		let lens = match lens {
			Some(x) => x,
			None => return
		};
		
		let lens_handler = self.lenses.handlers.get_mut(lens_id);
		let lens_handler = match lens_handler {
			Some(x) => x,
			None => return
		};
		
		// A lens can only receive an event if inactive or the event is PASSIVE.
		if !event.is_passive() {
			if lens.state != lens::LensState::Idle {
				return
			}
		}
		
		// A lens without path cannot receive events
		if lens.path.len() == 0 {
			return;
		}
		
		// Holder for event state.
		let mut event_wrapper = EventWrapper {
			event,
			
			// Initial State
			phase: EventPhase::Creation,
			can_propagate: true,
			can_default: true,
			can_bubble: true,
		};
		
		// --- Event Propagation
		event_wrapper.phase = EventPhase::Propagation;
		for node_id in lens.path.iter() {
			self.nodes.get_mut_node_by_id(*node_id).map(|n|
				n.on_event(&mut event_wrapper)
			);
			
			if !event_wrapper.can_propagate {
				break;
			}
		}
		
		// --- Event Action
		let new_state = if event_wrapper.can_default {
			event_wrapper.phase = EventPhase::Action;
			
			(*lens_handler).on_event(&mut event_wrapper, lens)
		} else {
			lens::LensState::Idle
		};
		
		// --- Event Bubbling
		if event_wrapper.can_bubble {
			event_wrapper.phase = EventPhase::Bubbling;
			for node_id in lens.path.iter().rev() {
				self.nodes.get_mut_node_by_id(*node_id).map(|n|
					n.on_event(&mut event_wrapper)
				);
				
				if !event_wrapper.can_bubble {
					break;
				}
			}
		}
		
		if lens.state != lens::LensState::Idle {
			// Don't start a new action if one is already running!
			return
		}
		
		// Swap in the action, kicking off whatever action the lens wants...
		lens.state = new_state
	}
	
	/// Fires a single `Event` at a single `Node`, given a path.
	#[allow(unused)]
	pub fn fire_event_at_node(&mut self, path: &str, event: &mut Event) {
		/*
		let mut path_offset = 0;
		let mut src_path = vec![];
		let mut bubble_path = vec![];
		
		loop {
			
			let step = Router::path_next(
				&self.nodes.nodes,
				path,
				&mut path_offset,
				src_path.as_slice()
			);
			
			// TODO: Implement firing of events at nodes.
		}
		*/
	}
}
