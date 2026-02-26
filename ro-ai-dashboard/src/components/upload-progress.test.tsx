import React from 'react';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { UploadProgress } from './upload-progress';

describe('UploadProgress', () => {
    // UT-F01e: Upload in progress → shows progress bar + % per file
    it('UT-F01e: displays progress bar with percentage for each file', () => {
        const files = [
            { name: 'report.pdf', progress: 45, status: 'uploading' as const },
            { name: 'data.csv', progress: 100, status: 'complete' as const },
            { name: 'notes.txt', progress: 0, status: 'pending' as const },
        ];

        render(<UploadProgress files={files} />);

        // Each file name should be displayed
        expect(screen.getByText('report.pdf')).toBeInTheDocument();
        expect(screen.getByText('data.csv')).toBeInTheDocument();
        expect(screen.getByText('notes.txt')).toBeInTheDocument();

        // Progress percentages should be displayed
        expect(screen.getByText('45%')).toBeInTheDocument();
        expect(screen.getByText('100%')).toBeInTheDocument();
        expect(screen.getByText('0%')).toBeInTheDocument();

        // Progress bars should exist (one per file)
        const progressBars = screen.getAllByRole('progressbar');
        expect(progressBars).toHaveLength(3);
    });
});
