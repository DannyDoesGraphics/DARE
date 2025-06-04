// Example usage of UniqueSlotMap
use dare_containers::prelude::*;

fn main() {
    let mut unique_map: UniqueSlotMap<i32> = UniqueSlotMap::default();

    // Insert some unique values
    let slot1 = unique_map.insert(42).expect("Should insert successfully");
    let slot2 = unique_map.insert(100).expect("Should insert successfully");

    println!("Inserted 42 at slot: {:?}", slot1);
    println!("Inserted 100 at slot: {:?}", slot2);

    // Try to insert a duplicate - this should fail
    match unique_map.insert(42) {
        Ok(_) => println!("ERROR: Duplicate was inserted!"),
        Err(error) => println!("Correctly rejected duplicate: {}", error),
    }

    // Check if values exist
    println!("Contains 42: {}", unique_map.contains_value(&42));
    println!("Contains 999: {}", unique_map.contains_value(&999));

    // Get slot for a value
    if let Some(slot) = unique_map.get_slot_for_value(&42) {
        println!("Found slot for 42: {:?}", slot);
        println!("Value at slot: {:?}", unique_map.get(*slot));
    }

    // Remove a value and try to insert it again
    let removed = unique_map
        .remove(slot1)
        .expect("Should remove successfully");
    println!("Removed value: {}", removed);

    // Now we should be able to insert 42 again
    let new_slot = unique_map
        .insert(42)
        .expect("Should insert successfully after removal");
    println!("Re-inserted 42 at new slot: {:?}", new_slot);
}
