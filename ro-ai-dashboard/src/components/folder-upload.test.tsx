import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { FolderUpload } from './folder-upload';

describe('FolderUpload', () => {
    // UT-F01d: Select folder with 3 files → shows 3 files in list
    it('UT-F01d: displays 3 files when a folder with 3 files is selected', async () => {
        const onFilesSelected = jest.fn();
        render(<FolderUpload onFilesSelected={onFilesSelected} />);

        const input = screen.getByTestId('folder-input');

        // Create 3 mock files simulating a folder selection
        const files = [
            new File(['content1'], 'doc1.pdf', { type: 'application/pdf' }),
            new File(['content2'], 'data.csv', { type: 'text/csv' }),
            new File(['content3'], 'notes.txt', { type: 'text/plain' }),
        ];

        // Simulate folder selection via file input change
        fireEvent.change(input, { target: { files } });

        await waitFor(() => {
            expect(screen.getByText('doc1.pdf')).toBeInTheDocument();
            expect(screen.getByText('data.csv')).toBeInTheDocument();
            expect(screen.getByText('notes.txt')).toBeInTheDocument();
        });

        expect(onFilesSelected).toHaveBeenCalledWith(
            expect.arrayContaining([
                expect.objectContaining({ name: 'doc1.pdf' }),
                expect.objectContaining({ name: 'data.csv' }),
                expect.objectContaining({ name: 'notes.txt' }),
            ])
        );
    });
});
