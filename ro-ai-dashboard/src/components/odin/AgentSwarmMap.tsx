'use client';

import React, { useEffect, useRef } from 'react';
import * as d3 from 'd3';

interface Node {
  id: number;
  name: string;
  display_name: string;
  status: string;
  model_id: string;
  latency_ms: number;
  tier: number;
  is_online: boolean;
}

interface Link {
  source: number | Node;
  target: number | Node;
  latency_ms: number;
  dispatch_count: number;
  success_rate: number;
}

interface AgentSwarmMapProps {
  nodes: Node[];
  links: Link[];
  onNodeClick?: (node: Node) => void;
  onEdgeHover?: (edge: Link) => void;
}

export const AgentSwarmMap: React.FC<AgentSwarmMapProps> = ({
  nodes,
  links,
  onNodeClick,
  onEdgeHover,
}) => {
  const svgRef = useRef<SVGSVGElement>(null);
  const simulationRef = useRef<d3.Simulation<Node, Link> | null>(null);

  useEffect(() => {
    if (!svgRef.current || nodes.length === 0) return;

    const width = svgRef.current.clientWidth || 800;
    const height = svgRef.current.clientHeight || 600;

    // Clear previous content
    d3.select(svgRef.current).selectAll('*').remove();

    // Create SVG groups
    const svg = d3.select(svgRef.current)
      .attr('width', width)
      .attr('height', height);

    const g = svg.append('g');

    // Create force simulation
    const simulation = d3.forceSimulation<Node>(nodes)
      .force('link', d3.forceLink<Node, Link>(links)
        .id((d: any) => d.id)
        .distance(100))
      .force('charge', d3.forceManyBody().strength(-300))
      .force('center', d3.forceCenter(width / 2, height / 2));

    simulationRef.current = simulation;

    // Draw links
    const link = g.selectAll('line')
      .data(links)
      .enter()
      .append('line')
      .attr('stroke', '#999')
      .attr('stroke-opacity', 0.6)
      .attr('stroke-width', (d: any) => Math.sqrt(d.dispatch_count) * 2)
      .on('mouseenter', (event, d: any) => onEdgeHover?.(d))
      .on('mouseleave', () => onEdgeHover?.(null as any));

    // Draw nodes
    const node = g.selectAll('circle')
      .data(nodes)
      .enter()
      .append('circle')
      .attr('r', (d: Node) => {
        // Tier 1 (Odin) larger, tier 2 (Frigg) medium, tier 3+ smaller
        return 30 + (4 - d.tier) * 10;
      })
      .attr('fill', (d: Node) => {
        if (d.tier === 1) return '#FF6B6B'; // Odin - red
        if (d.tier === 2) return '#4ECDC4'; // Frigg - teal
        return '#95E1D3'; // Specialists - light
      })
      .attr('opacity', (d: Node) => d.is_online ? 1 : 0.5)
      .style('cursor', 'pointer')
      .on('click', (event, d: Node) => onNodeClick?.(d))
      .call(drag(simulation));

    // Add labels
    const labels = g.selectAll('text')
      .data(nodes)
      .enter()
      .append('text')
      .text((d: Node) => d.name)
      .attr('font-size', '12px')
      .attr('text-anchor', 'middle')
      .attr('dy', '.3em')
      .attr('pointer-events', 'none')
      .attr('fill', '#fff')
      .attr('font-weight', 'bold');

    // Add status indicators
    const statusCircles = g.selectAll('.status-indicator')
      .data(nodes)
      .enter()
      .append('circle')
      .attr('class', 'status-indicator')
      .attr('r', 5)
      .attr('fill', (d: Node) => d.status === 'active' ? '#4CAF50' : '#FF9800')
      .attr('cx', (d: any) => 0)
      .attr('cy', (d: any) => -20);

    // Update positions on tick
    simulation.on('tick', () => {
      link
        .attr('x1', (d: any) => d.source.x)
        .attr('y1', (d: any) => d.source.y)
        .attr('x2', (d: any) => d.target.x)
        .attr('y2', (d: any) => d.target.y);

      node
        .attr('cx', (d: any) => d.x)
        .attr('cy', (d: any) => d.y);

      labels
        .attr('x', (d: any) => d.x)
        .attr('y', (d: any) => d.y);

      statusCircles
        .attr('transform', (d: any) => `translate(${d.x},${d.y})`);
    });

    // Zoom functionality
    const zoom = d3.zoom<SVGSVGElement, unknown>()
      .on('zoom', (event) => {
        g.attr('transform', event.transform);
      });

    svg.call(zoom as any);

    return () => {
      simulation.stop();
    };
  }, [nodes, links, onNodeClick, onEdgeHover]);

  function drag(simulation: d3.Simulation<Node, Link>) {
    function dragstarted(event: any) {
      if (!event.active) simulation.alphaTarget(0.3).restart();
      event.subject.fx = event.subject.x;
      event.subject.fy = event.subject.y;
    }

    function dragged(event: any) {
      event.subject.fx = event.x;
      event.subject.fy = event.y;
    }

    function dragended(event: any) {
      if (!event.active) simulation.alphaTarget(0);
      event.subject.fx = null;
      event.subject.fy = null;
    }

    return d3.drag()
      .on('start', dragstarted)
      .on('drag', dragged)
      .on('end', dragended);
  }

  return (
    <div className="w-full h-full bg-gradient-to-b from-slate-900 to-slate-800 rounded-lg border border-slate-700">
      <svg
        ref={svgRef}
        className="w-full h-full"
        style={{ minHeight: '400px' }}
      />
    </div>
  );
};
