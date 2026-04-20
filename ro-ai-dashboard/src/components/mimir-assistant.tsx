"use client";

import { useState, useRef, useEffect } from "react";
import { MessageCircle, X, Send, Bot, Loader2 } from "lucide-react";
import ReactMarkdown from "react-markdown";

type Message = {
    role: "system" | "user" | "assistant";
    content: string;
};

export function MimirAssistant() {
    const [isOpen, setIsOpen] = useState(false);
    const [messages, setMessages] = useState<Message[]>([
        { role: "assistant", content: "Hi! I'm the Mimir Helpdesk Assistant. Need help understanding RAG parameters or how to build an Agent? Just ask!" }
    ]);
    const [input, setInput] = useState("");
    const [isLoading, setIsLoading] = useState(false);
    const endOfMessagesRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (isOpen) {
            endOfMessagesRef.current?.scrollIntoView({ behavior: "smooth" });
        }
    }, [messages, isOpen]);

    const handleSend = async () => {
        if (!input.trim() || isLoading) return;
        const userMsg: Message = { role: "user", content: input.trim() };
        
        setMessages(prev => [...prev, userMsg]);
        setInput("");
        setIsLoading(true);

        try {
            const apiMessages = [...messages, userMsg].filter(m => m.role !== "system");
            const res = await fetch("/api/assistant", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ messages: apiMessages })
            });

            if (!res.ok) throw new Error("Failed to get response");
            const data = await res.json();
            
            setMessages(prev => [...prev, { role: "assistant", content: data.reply }]);
        } catch (error) {
            setMessages(prev => [...prev, { role: "assistant", content: "Sorry, I'm having trouble connecting to the Heimdall network right now." }]);
        } finally {
            setIsLoading(false);
        }
    };

    return (
        <div className="fixed bottom-6 right-6 z-50 flex flex-col items-end">
            {/* Chat Box */}
            {isOpen && (
                <div className="bg-white dark:bg-zinc-950 w-80 sm:w-96 rounded-2xl shadow-2xl border border-indigo-100 dark:border-zinc-800 mb-4 overflow-hidden flex flex-col h-[500px] max-h-[70vh] transition-all transform origin-bottom-right">
                    <div className="bg-gradient-to-r from-indigo-600 to-purple-600 p-4 flex justify-between items-center text-white">
                        <div className="flex items-center gap-2">
                            <Bot className="w-5 h-5" />
                            <h3 className="font-semibold text-sm">Mimir Assistant</h3>
                        </div>
                        <button onClick={() => setIsOpen(false)} className="hover:bg-white/20 p-1 rounded-lg transition-colors">
                            <X className="w-4 h-4" />
                        </button>
                    </div>
                    
                    <div className="flex-1 overflow-y-auto p-4 space-y-4 bg-gray-50/50 dark:bg-black/20">
                        {messages.map((msg, idx) => (
                            <div key={idx} className={`flex ${msg.role === "user" ? "justify-end" : "justify-start"}`}>
                                <div className={`max-w-[85%] rounded-2xl px-4 py-2.5 text-sm ${msg.role === "user" ? "bg-indigo-600 text-white" : "bg-white dark:bg-zinc-900 border border-gray-100 dark:border-zinc-800 text-gray-800 dark:text-zinc-200 shadow-sm"}`}>
                                    <div className="prose prose-sm dark:prose-invert prose-p:leading-relaxed prose-pre:bg-zinc-800 prose-pre:text-xs">
                                        <ReactMarkdown>
                                            {msg.content}
                                        </ReactMarkdown>
                                    </div>
                                </div>
                            </div>
                        ))}
                        {isLoading && (
                            <div className="flex justify-start">
                                <div className="bg-white dark:bg-zinc-900 border border-gray-100 dark:border-zinc-800 rounded-2xl px-4 py-3 shadow-sm">
                                    <Loader2 className="w-4 h-4 animate-spin text-indigo-500" />
                                </div>
                            </div>
                        )}
                        <div ref={endOfMessagesRef} />
                    </div>

                    <div className="p-3 bg-white dark:bg-zinc-950 border-t border-gray-100 dark:border-zinc-800">
                        <div className="relative flex items-center">
                            <input 
                                type="text"
                                value={input}
                                onChange={e => setInput(e.target.value)}
                                onKeyDown={e => e.key === "Enter" && handleSend()}
                                placeholder="Ask about Mimir features..."
                                className="w-full pl-4 pr-10 py-2.5 bg-gray-100 dark:bg-zinc-900 border-none rounded-xl text-sm focus:ring-2 focus:ring-indigo-500 transition-all dark:text-white"
                            />
                            <button 
                                onClick={handleSend}
                                disabled={!input.trim() || isLoading}
                                className="absolute right-2 p-1.5 bg-indigo-600 text-white rounded-lg disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                            >
                                <Send className="w-3.5 h-3.5" />
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* Floating Button */}
            <button 
                onClick={() => setIsOpen(!isOpen)}
                className="w-14 h-14 bg-gradient-to-r from-indigo-600 to-purple-600 rounded-full shadow-lg hover:shadow-xl flex items-center justify-center text-white hover:scale-105 transition-all focus:outline-none focus:ring-4 focus:ring-indigo-500/30"
            >
                {isOpen ? <X className="w-6 h-6" /> : <MessageCircle className="w-6 h-6" />}
            </button>
        </div>
    );
}
