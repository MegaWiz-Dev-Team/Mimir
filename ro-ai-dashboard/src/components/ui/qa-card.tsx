import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { QAResult } from "@/types/pipeline";

interface QACardProps {
    qa: QAResult;
}

export function QACard({ qa }: QACardProps) {
    return (
        <Card className="mb-4">
            <CardHeader className="pb-2">
                <CardTitle className="text-base font-semibold text-blue-600">Q: {qa.question}</CardTitle>
            </CardHeader>
            <CardContent>
                <p className="mb-4 text-sm text-foreground">A: {qa.answer}</p>

                {qa.context && (
                    <Accordion type="single" collapsible>
                        <AccordionItem value="context" className="border-b-0">
                            <AccordionTrigger className="py-2 text-xs text-muted-foreground hover:text-foreground">
                                Show Context
                            </AccordionTrigger>
                            <AccordionContent className="text-xs text-muted-foreground bg-muted p-2 rounded-md">
                                {qa.context}
                            </AccordionContent>
                        </AccordionItem>
                    </Accordion>
                )}
            </CardContent>
        </Card>
    );
}
