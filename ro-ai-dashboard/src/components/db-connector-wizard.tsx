"use client";

import { useState } from "react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Database, CheckCircle2, XCircle, Loader2, ArrowLeft, ArrowRight, Table2, Search } from "lucide-react";
import { testDbConnection, discoverDbSchema, importDbData, DbConnectionConfig, DbTableSchema, DbTestResult } from "@/lib/api";

type DbType = "mysql" | "postgres" | "sqlite";

interface DbConnectorWizardProps {
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onImportComplete?: (markdown: string, rowCount: number) => void;
}

const DB_TYPES: { value: DbType; label: string; icon: string; placeholder: string }[] = [
    { value: "mysql", label: "MySQL", icon: "🐬", placeholder: "mysql://user:pass@host:3306/database" },
    { value: "postgres", label: "PostgreSQL", icon: "🐘", placeholder: "postgres://user:pass@host:5432/database" },
    { value: "sqlite", label: "SQLite", icon: "📦", placeholder: "/path/to/database.db" },
];

export function DbConnectorWizard({ open, onOpenChange, onImportComplete }: DbConnectorWizardProps) {
    const [step, setStep] = useState<1 | 2 | 3>(1);
    const [dbType, setDbType] = useState<DbType | null>(null);
    const [name, setName] = useState("");
    const [connectionString, setConnectionString] = useState("");

    // Step 2 state
    const [testing, setTesting] = useState(false);
    const [testResult, setTestResult] = useState<DbTestResult | null>(null);
    const [discovering, setDiscovering] = useState(false);
    const [tables, setTables] = useState<DbTableSchema[]>([]);

    // Step 3 state
    const [query, setQuery] = useState("");
    const [importing, setImporting] = useState(false);
    const [importPreview, setImportPreview] = useState<string | null>(null);

    const reset = () => {
        setStep(1);
        setDbType(null);
        setName("");
        setConnectionString("");
        setTesting(false);
        setTestResult(null);
        setDiscovering(false);
        setTables([]);
        setQuery("");
        setImporting(false);
        setImportPreview(null);
    };

    const getConfig = (): DbConnectionConfig => ({
        name,
        db_type: dbType!,
        connection_string: connectionString,
    });

    const handleTest = async () => {
        setTesting(true);
        setTestResult(null);
        try {
            const result = await testDbConnection(getConfig());
            setTestResult(result);
        } catch (error: any) {
            setTestResult({ success: false, error: error.message });
        } finally {
            setTesting(false);
        }
    };

    const handleDiscover = async () => {
        setDiscovering(true);
        try {
            const result = await discoverDbSchema(getConfig());
            setTables(result.tables);
        } catch (error) {
            console.warn("[DB Wizard] Schema discovery failed:", error);
        } finally {
            setDiscovering(false);
        }
    };

    const handleImport = async () => {
        if (!query.trim()) return;
        setImporting(true);
        try {
            const result = await importDbData({ ...getConfig(), query });
            setImportPreview(result.markdown);
            onImportComplete?.(result.markdown, result.row_count);
        } catch (error) {
            console.warn("[DB Wizard] Import failed:", error);
        } finally {
            setImporting(false);
        }
    };

    const canProceed = () => {
        if (step === 1) return dbType && name.trim() && connectionString.trim();
        if (step === 2) return testResult?.success;
        return false;
    };

    return (
        <Dialog open={open} onOpenChange={(o) => { if (!o) reset(); onOpenChange(o); }}>
            <DialogContent className="sm:max-w-lg" data-testid="db-connector-wizard">
                <DialogHeader>
                    <DialogTitle className="flex items-center gap-2">
                        <Database className="w-5 h-5 text-purple-500" />
                        External Database Import
                    </DialogTitle>
                    <DialogDescription>
                        Step {step} of 3 — {step === 1 ? "Connection Details" : step === 2 ? "Test & Discover" : "Query & Import"}
                    </DialogDescription>
                    {/* Step indicator */}
                    <div className="flex items-center gap-2 pt-2">
                        {[1, 2, 3].map((s) => (
                            <div key={s} className={`h-1.5 flex-1 rounded-full transition-colors ${s <= step ? "bg-purple-500" : "bg-muted"}`} />
                        ))}
                    </div>
                </DialogHeader>

                <div className="py-4 space-y-4">
                    {/* Step 1: Connection Details */}
                    {step === 1 && (
                        <>
                            <div>
                                <Label className="text-sm font-medium mb-2 block">Database Type</Label>
                                <div className="grid grid-cols-3 gap-2">
                                    {DB_TYPES.map((db) => (
                                        <button
                                            key={db.value}
                                            type="button"
                                            data-testid={`db-type-${db.value}`}
                                            onClick={() => setDbType(db.value)}
                                            className={`p-3 rounded-lg border text-center text-sm transition-all ${dbType === db.value
                                                    ? "border-purple-500 bg-purple-50 dark:bg-purple-950/30 ring-1 ring-purple-500"
                                                    : "hover:bg-muted/50"
                                                }`}
                                        >
                                            <div className="text-2xl mb-1">{db.icon}</div>
                                            <div className="font-medium">{db.label}</div>
                                        </button>
                                    ))}
                                </div>
                            </div>

                            <div className="grid gap-2">
                                <Label htmlFor="db-name">Connection Name</Label>
                                <Input id="db-name" value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Production Analytics" />
                            </div>

                            <div className="grid gap-2">
                                <Label htmlFor="db-conn">Connection String</Label>
                                <Input
                                    id="db-conn"
                                    value={connectionString}
                                    onChange={(e) => setConnectionString(e.target.value)}
                                    placeholder={dbType ? DB_TYPES.find(d => d.value === dbType)?.placeholder : "Select a database type first"}
                                    className="font-mono text-sm"
                                />
                            </div>
                        </>
                    )}

                    {/* Step 2: Test & Discover */}
                    {step === 2 && (
                        <>
                            <div className="flex items-center gap-3">
                                <Button onClick={handleTest} disabled={testing} variant="outline" data-testid="test-connection-btn">
                                    {testing ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : <Database className="w-4 h-4 mr-2" />}
                                    Test Connection
                                </Button>

                                {testResult && (
                                    <div className={`flex items-center gap-2 text-sm ${testResult.success ? "text-green-600" : "text-red-600"}`}>
                                        {testResult.success ? (
                                            <><CheckCircle2 className="w-4 h-4" /> Connected — {testResult.version}</>
                                        ) : (
                                            <><XCircle className="w-4 h-4" /> {testResult.error}</>
                                        )}
                                    </div>
                                )}
                            </div>

                            {testResult?.success && (
                                <div>
                                    <Button onClick={handleDiscover} disabled={discovering} variant="outline" size="sm" data-testid="discover-schema-btn">
                                        {discovering ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : <Search className="w-4 h-4 mr-2" />}
                                        Discover Schema
                                    </Button>

                                    {tables.length > 0 && (
                                        <div className="mt-3 border rounded-md p-3 max-h-48 overflow-y-auto bg-muted/30">
                                            <div className="text-sm font-medium mb-2">{tables.length} Tables Found</div>
                                            {tables.map((t) => (
                                                <div key={t.table_name} className="flex items-center gap-2 py-1 text-sm">
                                                    <Table2 className="w-3.5 h-3.5 text-muted-foreground" />
                                                    <span className="font-mono">{t.table_name}</span>
                                                    <span className="text-xs text-muted-foreground">({t.columns.length} cols)</span>
                                                </div>
                                            ))}
                                        </div>
                                    )}
                                </div>
                            )}
                        </>
                    )}

                    {/* Step 3: Query & Import */}
                    {step === 3 && (
                        <>
                            <div className="grid gap-2">
                                <Label htmlFor="db-query">SQL Query (SELECT only)</Label>
                                <textarea
                                    id="db-query"
                                    value={query}
                                    onChange={(e) => setQuery(e.target.value)}
                                    placeholder="SELECT * FROM users LIMIT 100"
                                    rows={4}
                                    className="w-full px-3 py-2 text-sm font-mono border rounded-lg bg-white dark:bg-zinc-900 dark:border-zinc-700 focus:ring-2 focus:ring-purple-500 outline-none resize-none"
                                />
                            </div>

                            <Button onClick={handleImport} disabled={importing || !query.trim()} data-testid="import-btn">
                                {importing ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : <Database className="w-4 h-4 mr-2" />}
                                Run & Import
                            </Button>

                            {importPreview && (
                                <div className="border rounded-md p-3 max-h-48 overflow-auto bg-muted/30">
                                    <div className="text-sm font-medium mb-2">Import Preview</div>
                                    <pre className="text-xs font-mono whitespace-pre-wrap">{importPreview}</pre>
                                </div>
                            )}
                        </>
                    )}
                </div>

                <DialogFooter className="flex-row justify-between gap-2">
                    {step > 1 && (
                        <Button variant="outline" onClick={() => setStep((s) => Math.max(1, s - 1) as 1 | 2 | 3)}>
                            <ArrowLeft className="w-4 h-4 mr-1" /> Back
                        </Button>
                    )}
                    <div className="flex-1" />
                    {step < 3 && (
                        <Button onClick={() => setStep((s) => Math.min(3, s + 1) as 1 | 2 | 3)} disabled={!canProceed()}>
                            Next <ArrowRight className="w-4 h-4 ml-1" />
                        </Button>
                    )}
                    {step === 3 && importPreview && (
                        <Button onClick={() => { reset(); onOpenChange(false); }}>
                            Done
                        </Button>
                    )}
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
