"use client";

import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Building2, Save } from "lucide-react";
import { SettingsTabProps } from "./types";

export function GeneralTab({ isLoading, isSaving, currentTenantId, tenantName, setTenantName, handleSave }: SettingsTabProps) {
    return (
        <Card>
            <CardHeader>
                <CardTitle className="flex items-center gap-2">
                    <Building2 className="w-5 h-5 text-primary" />
                    Tenant Configuration
                </CardTitle>
                <CardDescription>
                    Update the name of your organization/tenant. This name is displayed across the platform.
                </CardDescription>
            </CardHeader>
            <CardContent>
                {isLoading ? (
                    <div className="py-4 text-center text-muted-foreground">Loading settings...</div>
                ) : currentTenantId ? (
                    <form onSubmit={handleSave} className="space-y-6">
                        <div className="space-y-2">
                            <label className="text-sm font-medium">Tenant Name</label>
                            <Input
                                required
                                placeholder="Enter tenant name"
                                value={tenantName}
                                onChange={e => setTenantName(e.target.value)}
                            />
                        </div>

                        <div className="space-y-2">
                            <label className="text-sm font-medium text-muted-foreground">Tenant ID</label>
                            <Input
                                disabled
                                value={currentTenantId}
                                className="bg-muted cursor-not-allowed font-mono text-xs"
                            />
                            <p className="text-xs text-muted-foreground">The internal ID cannot be modified.</p>
                        </div>

                        <div className="pt-4 flex justify-end">
                            <Button type="submit" disabled={isSaving || !tenantName.trim()}>
                                <Save className="w-4 h-4 mr-2" />
                                {isSaving ? "Saving..." : "Save Changes"}
                            </Button>
                        </div>
                    </form>
                ) : (
                    <div className="py-4 text-center text-muted-foreground">No tenant data found.</div>
                )}
            </CardContent>
        </Card>
    );
}
