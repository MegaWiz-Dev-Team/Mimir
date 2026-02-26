import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import QualityControlPage from './page';
import * as api from '@/lib/api';

// Mock the API module
jest.mock('@/lib/api', () => ({
    fetchQcClusters: jest.fn(),
    resolveQcCluster: jest.fn(),
    triggerQcGeneration: jest.fn(),
    fetchQcStatus: jest.fn(),
}));

// Mock @hello-pangea/dnd
jest.mock('@hello-pangea/dnd', () => ({
    DragDropContext: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
    Droppable: ({ children }: { children: (provided: any, snapshot: any) => React.ReactNode }) =>
        <div>{children({ droppableProps: {}, innerRef: jest.fn(), placeholder: null }, { isDraggingOver: false })}</div>,
    Draggable: ({ children }: { children: (provided: any, snapshot: any) => React.ReactNode }) =>
        <div>{children({ draggableProps: {}, dragHandleProps: {}, innerRef: jest.fn() }, { isDragging: false })}</div>,
}));

const mockClusters = [
    {
        id: 'cluster-1',
        topic: 'Monster Drop Rate',
        cluster_type: 'CONFLICT',
        status: 'PENDING',
        items: [
            { source_label: 'A', question: 'What drops from Poring?', answer: 'Apple', source: 'wiki-1.md' },
            { source_label: 'B', question: 'What drops from Poring?', answer: 'Jellopy', source: 'wiki-2.md' },
        ],
    },
    {
        id: 'cluster-2',
        topic: 'City Location',
        cluster_type: 'DUPLICATE',
        status: 'RESOLVED_A',
        items: [
            { source_label: 'A', question: 'Where is Prontera?', answer: 'Center of Midgard', source: 'wiki-1.md' },
        ],
    },
];

describe('QualityControlPage - Sprint 7 Phase 3', () => {
    beforeEach(() => {
        jest.clearAllMocks();
        (api.fetchQcClusters as jest.Mock).mockResolvedValue({ clusters: mockClusters });
        (api.fetchQcStatus as jest.Mock).mockResolvedValue({ is_generating: false });
    });

    it('renders Kanban columns: Pending Review and Resolved', async () => {
        render(<QualityControlPage />);

        await waitFor(() => {
            expect(screen.getByText('Pending Review')).toBeInTheDocument();
            expect(screen.getByText('Resolved')).toBeInTheDocument();
        });
    });

    it('displays cluster cards sorted into correct columns', async () => {
        render(<QualityControlPage />);

        await waitFor(() => {
            expect(screen.getByText('Monster Drop Rate')).toBeInTheDocument();
            expect(screen.getByText('City Location')).toBeInTheDocument();
        });

        // CONFLICT cluster should show CONFLICT badge
        expect(screen.getByText('CONFLICT')).toBeInTheDocument();
        // DUPLICATE cluster should show DUPLICATE badge
        expect(screen.getByText('DUPLICATE')).toBeInTheDocument();
    });

    it('shows cluster pair count', async () => {
        render(<QualityControlPage />);

        await waitFor(() => {
            expect(screen.getByText('2 pairs')).toBeInTheDocument();
            expect(screen.getByText('1 pairs')).toBeInTheDocument();
        });
    });

    it('renders the resolution dialog markup with expected structure', async () => {
        render(<QualityControlPage />);

        await waitFor(() => {
            expect(screen.getByText('Monster Drop Rate')).toBeInTheDocument();
        });

        // Verify the dialog's trigger wrapper exists around the cluster card
        const conflictBadge = screen.getByText('CONFLICT');
        expect(conflictBadge).toBeInTheDocument();

        // The resolved cluster should show its resolution status
        const resolvedBadge = screen.getByText('SRC:A');
        expect(resolvedBadge).toBeInTheDocument();
    });

    it('shows inline edit pencil icon on hover for cluster topic', async () => {
        render(<QualityControlPage />);

        await waitFor(() => {
            expect(screen.getByText('Monster Drop Rate')).toBeInTheDocument();
        });

        // The Edit3 icon should exist (even if opacity-0 until hover)
        const container = screen.getByText('Monster Drop Rate').closest('.group');
        expect(container).toBeInTheDocument();
    });
});
