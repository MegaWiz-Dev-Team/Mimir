import { NextRequest, NextResponse } from "next/server";

export async function POST(req: NextRequest) {
    try {
        const { messages } = await req.json();

        // Inject the Mimir Assistant system prompt as the first message
        const systemMessage = {
            role: "system",
            content: "You are the Mimir Helpdesk Assistant, an integral part of the Asgard Mimir Platform. Your job is to help users navigate the Mimir Agent Studio. Explain technical concepts simply. E.g. 'RAG' means Retrieval-Augmented Generation. 'Top-K' means how many documents to retrieve. 'Vector Alpha' mixes keyword and semantic search. 'System Prompt' is the instruction set. Keep your answers concise, friendly, and in the language the user asks. Format your output using Markdown lists where appropriate."
        };

        // If the first message isn't a system prompt, insert it
        let payloadMessages = messages;
        if (messages.length > 0 && messages[0].role !== "system") {
            payloadMessages = [systemMessage, ...messages];
        }

        // Send to Heimdall (Running on the Mac Host via Orbstack/Docker network)
        const response = await fetch("http://host.docker.internal:8081/v1/chat/completions", {
            method: "POST",
            headers: {
                "Content-Type": "application/json",
                // "Authorization": "Bearer YOUR_TOKEN_IF_NEEDED"
            },
            body: JSON.stringify({
                model: "google/gemini-3-flash-preview", // Use a highly capable reasoning model for Mimir Assistant
                messages: payloadMessages,
                temperature: 0.3,
            }),
        });

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`Heimdall API error: ${errorText}`);
        }

        const data = await response.json();
        const reply = data.choices[0].message.content;

        return NextResponse.json({ reply });

    } catch (error: any) {
        console.error("Mimir Assistant Error:", error);
        return NextResponse.json({ error: error.message }, { status: 500 });
    }
}
