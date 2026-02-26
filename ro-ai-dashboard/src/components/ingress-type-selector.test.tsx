import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { IngressTypeSelector } from './ingress-type-selector';

describe('IngressTypeSelector', () => {
    // UT-F02a: Render → shows 4 cards
    it('UT-F02a: renders 4 ingress type cards', () => {
        const onSelect = jest.fn();
        render(<IngressTypeSelector onSelect={onSelect} />);

        expect(screen.getByText('Web Scraper')).toBeInTheDocument();
        expect(screen.getByText('Document Upload')).toBeInTheDocument();
        expect(screen.getByText('Tabular Data')).toBeInTheDocument();
        expect(screen.getByText('MCP Connection')).toBeInTheDocument();
    });

    // UT-F02b: Click "Document Upload" → calls onSelect('document')
    it('UT-F02b: calls onSelect with "document" when Document Upload is clicked', () => {
        const onSelect = jest.fn();
        render(<IngressTypeSelector onSelect={onSelect} />);

        fireEvent.click(screen.getByText('Document Upload'));

        expect(onSelect).toHaveBeenCalledWith('document');
    });
});
