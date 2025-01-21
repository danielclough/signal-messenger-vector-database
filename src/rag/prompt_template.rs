// Respond with yaml following this format:
// ```yaml
// -
//  title: <title>
//  description: <description>
//  tags: [<tag1>, <tag2>]
//  content: |
//   <content>

pub fn llama3(user_message: &str) -> String {
    let system_prompt = "You are a friendly and useful Chatbot. Be of assistance the best you can.
";

    format!(
        "
<|begin_of_text|>
<|start_header_id|>system<|end_header_id|>
{}<|eot_id|>

<|start_header_id|>user<|end_header_id|>
{}<|eot_id|>

<|start_header_id|>assistant<|end_header_id|>",
        system_prompt, user_message
    )
}

// format!("
// <|begin_of_text|>
// <|start_header_id|>system<|end_header_id|>
// {}<|eot_id|>

// <|start_header_id|>user<|end_header_id|>
// {}<|eot_id|>

// <|start_header_id|>assistant<|end_header_id|>
// {}<|eot_id|>

// <|start_header_id|>user<|end_header_id|>
// {}<|eot_id|>

// <|start_header_id|>assistant<|end_header_id|>", system_prompt, user_message_1, model_response_1, user_message_2)
