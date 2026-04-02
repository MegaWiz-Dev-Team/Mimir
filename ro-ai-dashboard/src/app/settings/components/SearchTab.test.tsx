import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { SearchTab } from './SearchTab';

describe('SearchTab', () => {
    const mockSetConfig = jest.fn();
    const mockUpdateTenantConfigFn = jest.fn().mockResolvedValue(true);

    const mockConfig = {
        tenant_id: "t1",
        llm_config: {
            embedding: { provider: "heimdall", model: "BAAI/bge-m3" }
        }
    };

    const baseProps: any = {
        config: mockConfig,
        setConfig: mockSetConfig,
        isSaving: false,
        currentTenantId: "t1",
        updateTenantConfigFn: mockUpdateTenantConfigFn,
        topK: 5,
        setTopK: jest.fn(),
        similarityThreshold: 0.7,
        setSimilarityThreshold: jest.fn(),
        searchMode: "hybrid",
        setSearchMode: jest.fn()
    };

    it('renders vector provider and model dropdowns from config.llm_config.embedding', () => {
        render(<SearchTab {...baseProps} />);
        expect(screen.getByText('Embedding Provider & Model')).toBeInTheDocument();
        expect(screen.getByDisplayValue('Heimdall (Self-Hosted)')).toBeInTheDocument();
        expect(screen.getByDisplayValue('BGE-M3 (MLX)')).toBeInTheDocument();
    });

    it('calls setConfig when embedding provider changes', () => {
        render(<SearchTab {...baseProps} />);
        
        const providerSelect = screen.getByDisplayValue('Heimdall (Self-Hosted)');
        fireEvent.change(providerSelect, { target: { value: 'openai' } });

        expect(mockSetConfig).toHaveBeenCalled();
    });
});
