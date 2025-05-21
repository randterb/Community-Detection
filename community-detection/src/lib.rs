use csv::Reader;
use petgraph::algo::tarjan_scc;
use petgraph::dot::{Dot, Config};
use petgraph::Graph;
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::Command;
use std::sync::{Arc, Mutex};

pub struct UsernameGenerator {
    prefixes: Vec<&'static str>,
    suffixes: Vec<&'static str>,
    used_names: Arc<Mutex<HashSet<String>>>,
}

impl UsernameGenerator {
    pub fn new() -> Self {
        UsernameGenerator {
            prefixes: vec![
                "dark", "shadow", "light", "blue", "red", "green", "gold", "silver",
                "phantom", "ninja", "stealth", "epic", "legend", "super", "mega",
            ],
            suffixes: vec![
                "warrior", "hunter", "mage", "slayer", "knight", "rogue", "wizard",
                "assassin", "lord", "king", "queen", "master", "pro", "noob", "gamer",
            ],
            used_names: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    // Parallel generation of unique usernames
    pub fn generate_unique_batch(&self, count: usize) -> Vec<String> {
        (0..count)
            .into_par_iter()
            .map_init(
                || thread_rng(),
                |rng, _| {
                    let prefix = self.prefixes.choose(rng).unwrap();
                    let suffix = self.suffixes.choose(rng).unwrap();
                    let num = rng.gen_range(1..999);
                    format!("{}{}{}", prefix, suffix, num)
                },
            )
            .filter(|name| {
                let mut used = self.used_names.lock().unwrap();
                used.insert(name.clone())
            })
            .collect()
    }
}

pub fn generate_interaction_csv(
    num_users: usize,
    num_interactions: usize,
    filename: &str,
) -> std::io::Result<()> {
    let generator = UsernameGenerator::new();
    let users = Arc::new(generator.generate_unique_batch(num_users));

    let file = File::create(filename)?;
    let writer = Arc::new(Mutex::new(BufWriter::new(file)));

    (0..num_interactions).into_par_iter().for_each(|_| {
        let mut rng = thread_rng();
        let user1 = users.choose(&mut rng).unwrap();
        let user2 = users.choose(&mut rng).unwrap();
        let weight = rng.gen_range(1..=20);

        let mut writer = writer.lock().unwrap();
        writeln!(writer, "{},{},{}", user1, user2, weight).unwrap();
    });

    Ok(())
}

pub struct CommunityDetector {
    pub graph: Graph<String, u32>,
    pub labels: HashMap<String, usize>,
}

impl CommunityDetector {
    pub fn from_csv(filename: &str) -> Result<Self, csv::Error> {
        let graph = Self::build_graph_from_csv_parallel(filename)?;
        let labels = HashMap::new();
        Ok(CommunityDetector { graph, labels })
    }

    // Parallel CSV parsing and graph construction
    pub fn build_graph_from_csv_parallel(filename: &str) -> Result<Graph<String, u32>, csv::Error> {
        let graph = Arc::new(Mutex::new(Graph::new()));
        let node_indices = Arc::new(Mutex::new(HashMap::new()));

        Reader::from_path(filename)?
            .into_records()
            .par_bridge() // Parallel bridge for rayon
            .for_each(|record| {
                let record = record.unwrap();
                let user1 = record[0].to_string();
                let user2 = record[1].to_string();
                let weight: u32 = record[2].parse().unwrap_or(1);

                let mut graph = graph.lock().unwrap();
                let mut node_indices = node_indices.lock().unwrap();

                let node1 = *node_indices
                    .entry(user1.clone())
                    .or_insert_with(|| graph.add_node(user1));
                let node2 = *node_indices
                    .entry(user2.clone())
                    .or_insert_with(|| graph.add_node(user2));

                if let Some(edge) = graph.find_edge(node1, node2) {
                    graph[edge] += weight;
                } else {
                    graph.add_edge(node1, node2, weight);
                }
            });

        Ok(Arc::try_unwrap(graph).unwrap().into_inner().unwrap())
    }

    pub fn detect_communities(&mut self) {
        let scc = tarjan_scc(&self.graph);
        
        // Parallel community labeling
        self.labels = scc
            .into_par_iter()
            .enumerate()
            .flat_map(|(community_id, nodes)| {
                nodes
                    .into_par_iter()
                    .map(|node| (self.graph[node].clone(), community_id))
                    .collect::<Vec<_>>()
            })
            .collect();
    }

    pub fn get_communities(&self) -> HashMap<usize, Vec<String>> {
        let mut communities = HashMap::new();
        for (user, &community_id) in &self.labels {
            communities
                .entry(community_id)
                .or_insert_with(Vec::new)
                .push(user.clone());
        }
        communities
    }

    pub fn save_graph_to_dot(
        &self,
        filename: &str,
    ) -> std::io::Result<()> {
        let node_to_community: HashMap<_, _> = self
            .labels
            .par_iter()
            .map(|(user, comm_id)| (user.clone(), *comm_id))
            .collect();

        let binding = |_, (_, username)| {
                let comm_id = node_to_community.get(username).unwrap();
                let hue = (comm_id * 60) % 360;
                format!(
                    "label=\"{}\", style=filled, fillcolor=\"{:.1} 0.5 0.7\"",
                    username, hue as f32
                )
            };
        let dot = Dot::with_attr_getters(
            &self.graph,
            &[Config::EdgeNoLabel],
            &|_, edge| format!("label=\"{}\"", edge.weight()),
            &binding,
        );

        std::fs::write(filename, format!("{:?}", dot))
    }

    pub fn render_and_open_graph(dot_file: &str, output_image: &str) -> std::io::Result<()> {
        Command::new("dot")
            .args(&["-Tpng", dot_file, "-o", output_image])
            .status()?;

        let opener = if cfg!(target_os = "windows") {
            "start"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };

        Command::new(opener).arg(output_image).status()?;
        Ok(())
    }
}

