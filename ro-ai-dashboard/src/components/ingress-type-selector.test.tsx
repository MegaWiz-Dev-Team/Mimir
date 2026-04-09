import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { IngressTypeSelector } from './ingress-type-selector';

describe('IngressTypeSelector', () => {
    // UT-F02a: Render → shows 3 cards (merged Document+Tabular into File Upload)
    it('UT-F02a: renders 3 ingress type cards', () => {
        const onSelect = jest.fn();
        render(<IngressTypeSelector onSelect={onSelect} />);

        expect(screen.getByText('File Upload')).toBeInTheDocument();
        expect(screen.getByText('Web Scraper')).toBeInTheDocument();
    });

    // UT-F02b: Click "File Upload" → calls onSelect('file')
    it('UT-F02b: calls onSelect with "file" when File Upload is clicked', () => {
        const onSelect = jest.fn();
        render(<IngressTypeSelector onSelect={onSelect} />);

        fireEvent.click(screen.getByText('File Upload'));

        expect(onSelect).toHaveBeenCalledWith('file');
    });

    // UT-087i: Document Upload and Tabular Data should NOT exist
    it('UT-087i: does not render old Document Upload and Tabular Data cards', () => {
        const onSelect = jest.fn();
        render(<IngressTypeSelector onSelect={onSelect} />);

        expect(screen.queryByText('Document Upload')).not.toBeInTheDocument();
        expect(screen.queryByText('Tabular Data')).not.toBeInTheDocument();
    });
});
