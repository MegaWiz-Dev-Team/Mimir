import React from 'react';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { AdvancedSettings } from './advanced-settings';

describe('AdvancedSettings', () => {
    const defaultProps = {
        ingressType: 'document' as const,
        settings: {
            ocrEnabled: false,
            useHeaderRow: true,
            storageMode: 'markdown' as const,
        },
        onSettingsChange: jest.fn(),
    };

    // UT-F02e: domain="medical" → shows OCR option
    it('UT-F02e: shows OCR option when domain is "medical"', () => {
        render(<AdvancedSettings {...defaultProps} domain="medical" />);

        expect(screen.getByText(/OCR/i)).toBeInTheDocument();
    });

    // UT-F02f: domain="game" → hides OCR option
    it('UT-F02f: hides OCR option when domain is "game"', () => {
        render(<AdvancedSettings {...defaultProps} domain="game" />);

        expect(screen.queryByText(/OCR/i)).not.toBeInTheDocument();
    });
});
