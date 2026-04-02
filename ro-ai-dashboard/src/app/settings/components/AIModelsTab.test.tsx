import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { AIModelsTab } from './AIModelsTab';

describe('AIModelsTab', () => {
    const mockSetConfig = jest.fn();
    const mockHandleSave = jest.fn();

    const mockConfig = {
        tenant_id: "t1",
        default_provider: "ollama",
        default_model: "llama3.2",
        max_daily_tokens: 100000,
        is_dedicated_vector_db: false,
        llm_config: {
            chat: { provider: "ollama", model: "llama3.2" },
            rag: { provider: "heimdall", model: "Qwen3.5-35B-A3B-4bit" },
            pipeline_generator: { provider: "gemini", model: "gemini-2.5-flash" },
            pipeline_evaluator: { provider: "gemini", model: "gemini-2.5-pro" },
            judge: { provider: "gemini", model: "gemini-2.5-flash" },
            embedding: { provider: "heimdall", model: "BAAI/bge-m3" },
        }
    };

    const baseProps: any = {
        isLoading: false,
        isSaving: false,
        config: mockConfig,
        setConfig: mockSetConfig,
        handleSave: mockHandleSave,
    };

    it('renders default provider and model selectors', () => {
        render(<AIModelsTab {...baseProps} />);
        expect(screen.getByText('Default Provider & Model')).toBeInTheDocument();
        const defaultProviderSelect = screen.getByDisplayValue('ollama'); // Assuming option 'ollama' is selected
        expect(defaultProviderSelect).toBeInTheDocument();
        const defaultModelSelect = screen.getAllByDisplayValue('llama3.2');
        expect(defaultModelSelect.length).toBeGreaterThan(0);
    });

    it('renders pipeline evaluator slot', () => {
        render(<AIModelsTab {...baseProps} />);
        expect(screen.getByText('Pipeline Evaluator')).toBeInTheDocument();
    });

    it('calls setConfig when default provider is changed', () => {
        render(<AIModelsTab {...baseProps} />);
        
        // Find the generic default provider select. We'll add an aria-label or accessible name to it in the code.
        const providerSelects = screen.getAllByRole('combobox');
        // Let's assume the first select is the Default Provider
        fireEvent.change(providerSelects[0], { target: { value: 'gemini' } });

        expect(mockSetConfig).toHaveBeenCalled();
    });
});
