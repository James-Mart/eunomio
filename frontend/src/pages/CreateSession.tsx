import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form";
import { api } from "@/lib/api";

const schema = z.object({
  baseRef: z.string().min(1, "baseRef is required"),
  sourceRef: z.string().min(1, "sourceRef is required"),
});

type FormValues = z.infer<typeof schema>;

export default function CreateSession() {
  const navigate = useNavigate();
  const [submitting, setSubmitting] = useState(false);
  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { baseRef: "main", sourceRef: "" },
  });

  const onSubmit = async (values: FormValues) => {
    setSubmitting(true);
    try {
      const session = await api.createSession(values.baseRef, values.sourceRef);
      navigate(`/sessions/${session.id}`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to create session");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="container max-w-md py-10">
      <Card>
        <CardHeader>
          <CardTitle>Create session</CardTitle>
          <CardDescription>
            Pick a base ref (the merge target) and a source ref (the work to review). Both must
            exist in the repo this server was started in.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Form {...form}>
            <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-4">
              <FormField
                control={form.control}
                name="baseRef"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>baseRef</FormLabel>
                    <FormControl>
                      <Input placeholder="main" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <FormField
                control={form.control}
                name="sourceRef"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>sourceRef</FormLabel>
                    <FormControl>
                      <Input placeholder="feature-branch" {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
              <Button type="submit" disabled={submitting} className="w-full">
                {submitting ? "Creating…" : "Create session"}
              </Button>
            </form>
          </Form>
        </CardContent>
      </Card>
    </div>
  );
}
