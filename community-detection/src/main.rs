// If the module is in the same directory as main.rs, use:
pub use community_detection::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Generate data
    generate_interaction_csv(140, 500, "interactions.csv")?;

    // 2. Build graph and detect communities
    let mut detector = CommunityDetector::from_csv("interactions.csv")?;
    detector.detect_communities();

    // 3. Save and visualize
    detector.save_graph_to_dot("graph.dot")?;
    CommunityDetector::render_and_open_graph("graph.dot", "graph.png")?;

    // 4. Print community info
    let communities = detector.get_communities();
    println!("Detected {} communities:", communities.len());
    for (id, members) in communities {
        println!("Community {} ({} members)", id, members.len());
    }

    Ok(())
}