# tracing-otel-spawn-example

## Background

This repo holds an example application using `tracing`, and is intended to be
a representative but simplified version of a real-world scenario where:

1. `parent` is always called, and may either re-invoke itself, or spawn `child`
1. `child` will do some work, and call `grandchild`
1. `grandchild` does some work

All of these binaries (`parent`, `child`, and `grandchild`) are instrumented
and, using `tracing_subscriber::fmt`, produce traces to STDERR.

## Goal

I'd like to start producing OpenTelemetry traces for this application, across
all its binaries, and to dump those traces to a JSON file.

## Issue

What is the correct way of achieving this? If all binaries were to write to a
JSON file, they would clobber each others writes as their execution happens at
the same time. Should `parent` start an OTEL collector, and all other binaries
should send their traces there? Is there a way to have tracing use a subscriber
from another process? Perhaps each can write to a unique JSON file and `parent`
can catenate them at the end?

Moreover, how do we keep the traces related, so we can know that a trace from
`grandchild` comes from an invocation of `child` which comes from an invocation
of `parent`?
