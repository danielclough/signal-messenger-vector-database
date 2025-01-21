use pgvector::Vector;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignalMessageWithEmbedding {
    pub body: String,
    pub direction: String,
    pub receiver: Option<String>,
    pub sender: Option<String>,
    pub group_name: Option<String>,
    pub attachments: Option<Vec<String>>,
    pub tokens: i32,
    pub embedding: Vec<f32>,
}

#[derive(Clone, Debug, FromRow, Encode)]
pub struct SignalMessageWithVector {
    pub body: String,
    pub direction: String,
    pub receiver: Option<String>,
    pub sender: Option<String>,
    pub group_name: Option<String>,
    pub attachments: Option<Vec<String>>,
    pub tokens: i32,
    pub embedding: Vector,
}
use sqlx::{Encode, FromRow};
use tiktoken_rs::cl100k_base;

use crate::signal::process_incoming_message::ProcessedMessage;

// Helper function to calculate number of tokens
fn num_tokens_from_str(string: &str) -> usize {
    if string.is_empty() {
        return 0;
    }
    let bpe = cl100k_base().unwrap();
    bpe.encode_with_special_tokens(string).len()
}

// // Helper function to calculate length of essay
// fn get_essay_length(essay: &str) -> usize {
//     essay.split_whitespace().count()
// }

pub async fn process_dataframe(df: &Vec<ProcessedMessage>) -> Vec<SignalMessageWithEmbedding> {
    let mut new_list = Vec::new();
    let ideal_token_size = 512;
    let ideal_size = ideal_token_size * 3 / 4;

    for data in df {
        let text = data.body.clone().unwrap_or(String::new());
        let token_len = num_tokens_from_str(&text.clone());

        if token_len <= 512 {
            new_list.push(SignalMessageWithEmbedding {
                body: text.clone(),
                tokens: token_len as i32,
                embedding: get_embeddings_from_ollama(&text).await.unwrap(),
                direction: data.direction.clone().unwrap().to_string(),
                receiver: data.receiver.clone(),
                sender: data.sender.clone(),
                group_name: data.group.clone(),
                attachments: data.attachments.clone(),
            });
        } else {
            let words: Vec<String> = text
                .clone()
                .split_whitespace()
                .map(|x| x.to_string())
                .collect();
            let total_words = words.len();
            let chunks = (total_words + ideal_size - 1) / ideal_size;

            for j in 0..chunks {
                let start = j * ideal_size;
                let end = (start + ideal_size).min(total_words);
                let new_body: Vec<String> = words[start..end].to_vec();
                let new_body_string = new_body.join(" ");
                let new_body_token_len = num_tokens_from_str(&new_body_string);

                let body = data.body.clone().unwrap_or(String::from("Error"));

                let embedding = match get_embeddings_from_ollama(&body).await {
                    Ok(x) => x,
                    Err(err) => {
                        panic!("err from ollama: {}", err);
                    }
                };
                // println!("embedding: {:?}", embedding);

                if new_body_token_len > 0 {
                    new_list.push(SignalMessageWithEmbedding {
                        body,
                        direction: match data.direction.clone() {
                            Some(x) => x.to_string(),
                            None => String::new(),
                        },
                        receiver: data.receiver.clone(),
                        sender: data.sender.clone(),
                        group_name: data.group.clone(),
                        attachments: data.attachments.clone(),
                        tokens: token_len as i32,
                        embedding,
                    });
                }
            }
        }
    }

    // println!("new_list: {:?}", new_list);
    new_list
}

async fn get_embeddings_from_ollama(text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let url = "http://localhost:11434/api/embeddings";

    let client = Client::new();

    let payload = json!({
        "model": "nomic-embed-text",
        "prompt": text.replace("\n", " ")
    });

    let response = match client
        .post(url)
        .header("body-Type", "application/json")
        .json(&payload)
        .send()
        .await {
            Ok(x) => Some(x),
            Err(err) => {
                println!("err: {:?}", err);
                None
            },
        };
    
    // println!("res: {:?}", response);

    if response.is_some() {
        let body: Value = response.unwrap().json().await?;
        let embedding = body["embedding"]
            .as_array()
            .ok_or("Embedding not found in response")?
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        println!("embedding: {:?}", embedding);
        Ok(embedding)
    } else {
        Err(format!("Error: {}", response.unwrap().text().await?).into())
    }
}

pub async fn process_message_to_get_embedding(
    data: Vec<ProcessedMessage>,
) -> Vec<SignalMessageWithEmbedding> {
    let mut new_list = process_dataframe(&data).await;

    // println!("{:?}", new_list);

    // Create embeddings
    for item in &mut new_list {
        let embedding = get_embeddings_from_ollama(&item.body).await.unwrap();
        item.embedding = embedding;
    }

    new_list
}

// // Create a new dataframe from the list
// let mut df_new = DataFrame::new(vec![
//     Series::new(
//         "body".into(),
//         new_list
//             .iter()
//             .map(|x| x.body.clone())
//             .collect::<Vec<String>>(),
//     ),
//     Series::new(
//         "direction".into(),
//         new_list
//             .iter()
//             .map(|x| x.direction.clone())
//             .collect::<Vec<String>>(),
//     ),
//     Series::new(
//         "receiver".into(),
//         new_list
//             .iter()
//             .map(|x| x.receiver.clone())
//             .collect::<Vec<Option<String>>>(),
//     ),
//     Series::new(
//         "sender".into(),
//         new_list
//             .iter()
//             .map(|x| x.sender.clone())
//             .collect::<Vec<Option<String>>>(),
//     ),
//     Series::new(
//         "group".into(),
//         new_list
//             .iter()
//             .map(|x| x.group_name.clone())
//             .collect::<Vec<Option<String>>>(),
//     ),
//     Series::new(
//         "attachments".into(),
//         new_list
//             .iter()
//             .map(|x| x.attachments.clone().unwrap_or(vec![]).join(","))
//             .collect::<Vec<String>>(),
//     ),
//     Series::new(
//         "tokens".into(),
//         new_list
//             .iter()
//             .map(|x| x.tokens as i32)
//             .collect::<Vec<i32>>(),
//     ),
//     Series::new(
//         "embedding".into(),
//         new_list
//             .iter()
//             .map(|x| Series::new("".into(), x.embedding.clone()))
//             .collect::<Vec<Series>>(),
//     ),
// ])
// .unwrap();

// // Print the first few rows of the dataframe
// println!("{}", df_new.head(Some(5)));
