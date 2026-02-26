import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { StorageModeSelector } from './storage-mode-selector';

describe('StorageModeSelector', () => {
    // UT-F02c: Render → shows Markdown (default) + SQL radio options
    it('UT-F02c: renders radio with Markdown selected by default', () => {
        const onChange = jest.fn();
        render(<StorageModeSelector value="markdown" onChange={onChange} />);

        const markdownRadio = screen.getByLabelText(/markdown/i);
        const sqlRadio = screen.getByLabelText(/sql table/i);

        expect(markdownRadio).toBeChecked();
        expect(sqlRadio).not.toBeChecked();
    });

    // UT-F02d: Select SQL → calls onChange('sql')
    it('UT-F02d: calls onChange with "sql" when SQL Table is selected', () => {
        const onChange = jest.fn();
        render(<StorageModeSelector value="markdown" onChange={onChange} />);

        const sqlRadio = screen.getByLabelText(/sql table/i);
        fireEvent.click(sqlRadio);

        expect(onChange).toHaveBeenCalledWith('sql');
    });
});
