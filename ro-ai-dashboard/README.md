# RO-AI Dashboard

The administrative interface for **Project Mimir**. This dashboard provides real-time visibility into the data pipeline, vector database health, and AI agent performance.

## ✨ Features

- **📊 Pipeline Monitoring**: 
  - Track active and historical pipeline runs.
  - **Live Status Diagram**: Visual flow of data from Ingestion -> Workshop -> Validation -> Storage.
  - **Resume & Retry**: Intelligent recovery for failed runs or specific chunks.
- **🗂️ Vector Database Management**:
  - **Sync Metrics**: Real-time stats comparing MariaDB record count vs Qdrant vector count.
  - **Search Preview**: Test RAG retrieval quality directly from the UI.
  - **Collection Health**: Monitor Qdrant collection status and indexing progress.
- **🔍 Q/A Exploration**:
  - Browse generated Q/A pairs.
  - View detailed **Evaluation Reports** including coverage scores and missing atomic facts.

## 🛠 Prerequisites

1.  **Node.js**: v18+ recommended.
2.  **AI Backend**: The `ro-ai-bridge` monitor server must be running (default: `http://localhost:8080`).

## 🚀 Getting Started

1.  **Install Dependencies**:
    ```bash
    npm install
    ```

2.  **Environment Setup**:
    Create a `.env.local` file:
    ```bash
    NEXT_PUBLIC_API_URL=http://localhost:8080/api
    ```

3.  **Run Development Server**:
    ```bash
    npm run dev
    ```

4.  **Production Build**:
    ```bash
    npm run build
    npm run start
    ```

## 🏗 Technology Stack

- **Framework**: [Next.js 15](https://nextjs.org) (App Router)
- **Styling**: Tailwind CSS
- **Components**: Shadcn/UI + Lucide React
- **Icons**: Lucide
- **State/Data**: Fetch + SWR/Effect hooks

---
*Created as part of the Project-Mimir AI Ecosystem.*
