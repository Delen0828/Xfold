export interface paths {
    "/api/v1/commits": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["commits"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/comparisons": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createComparison"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/comparisons/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["comparison"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/comparisons/{id}/changes": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["changes"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/comparisons/{id}/source-diff": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["sourceDiff"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/graph-views": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["graphView"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/graph-views/{id}/edges/{edge}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["edge"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/jobs/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["job"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/jobs/{id}/cancel": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["cancel"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/jobs/{id}/events": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["events"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/paths": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["path"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/refs": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["refs"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/snapshots": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["createSnapshot"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/snapshots/{id}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["snapshot"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/snapshots/{id}/children": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["children"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/snapshots/{id}/diagnostics": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["diagnostics"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/snapshots/{id}/modules-at": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["modulesAt"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/snapshots/{id}/modules/{module}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["module"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/snapshots/{id}/receipt": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["receipt"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/snapshots/{id}/search": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["search"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/snapshots/{id}/source": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["source"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/traversals": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post: operations["traversal"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/api/v1/workspace": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get: operations["workspace"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
}
export type webhooks = Record<string, never>;
export interface components {
    schemas: {
        BuildReceipt: {
            engine: string;
            graphHash: string;
            inputHash: string;
            snapshotId: string;
            stats: components["schemas"]["BuildStats"];
            topologyHash: string;
            validated: boolean;
        };
        BuildRequest: {
            force?: boolean;
            profile?: components["schemas"]["Profile"];
            revision: string;
        };
        BuildStats: {
            artifactBytes: number;
            /** Format: int64 */
            durationMs: number;
            edgesRederived: number;
            edgesReused: number;
            filesChanged: number;
            filesDeleted: number;
            filesParsed: number;
            filesResolved: number;
            filesReused: number;
            mode: string;
        };
        Change: {
            after?: string | null;
            before?: string | null;
            flags: string[];
            headline: boolean;
            id: string;
        };
        CompareRequest: {
            /** @description Empty means the target commit’s first parent (or an empty tree for a root commit). */
            base?: string;
            profile?: components["schemas"]["Profile"];
            target: string;
        };
        Comparison: {
            base: string;
            changes: components["schemas"]["Change"][];
            correspondence: {
                [key: string]: string;
            };
            diagnostics: components["schemas"]["Diagnostic"][];
            edges: components["schemas"]["EdgeChange"][];
            id: string;
            summary: {
                [key: string]: number;
            };
            target: string;
        };
        Dependency: {
            from: string;
            id: string;
            kind: string;
            provenance: string;
            to: string;
        };
        Diagnostic: {
            code: string;
            message: string;
            path?: string | null;
            severity: string;
        };
        EdgeChange: {
            edge: components["schemas"]["Dependency"];
            side: string;
            status: string;
        };
        FileRecord: {
            bytes: number;
            gitBlob: string;
            language: string;
            lines: number;
            object?: string | null;
            path: string;
            status: string;
        };
        GraphRequest: {
            changes?: string[];
            comparison?: string | null;
            edgeTypes?: string[];
            expanded?: string[];
            kinds?: string[];
            /** @description Restrict the projection to semantic changes, ancestors, and direct children. */
            light?: boolean;
            limit?: number | null;
            scope?: string | null;
            snapshot?: string | null;
        };
        GraphView: {
            breadcrumbs: components["schemas"]["Module"][];
            comparison?: string | null;
            edges: components["schemas"]["ViewEdge"][];
            id: string;
            layoutHash: string;
            nodes: components["schemas"]["ViewNode"][];
            omittedEdges: number;
            omittedNodes: number;
            scope: string;
            snapshot: string;
            truncated: boolean;
        };
        Job: {
            counters: {
                [key: string]: number;
            };
            error?: string | null;
            id: string;
            phase: string;
            resultId?: string | null;
            resultKind: string;
            /** Format: int64 */
            sequence: number;
            state: string;
        };
        Module: {
            contentHash: string;
            id: string;
            kappa?: string | null;
            kind: string;
            language: string;
            name: string;
            parent?: string | null;
            path: string;
            qualifiedName: string;
            semId?: string | null;
            source?: null | components["schemas"]["SourceRef"];
            structuralHash?: string | null;
        };
        PathRequest: {
            edgeTypes?: string[];
            from: string;
            snapshot: string;
            to: string;
        };
        Profile: {
            extensions?: string[];
            includeExcluded?: boolean;
        };
        ReadyOrJob: {
            id: string;
            kind: string;
            state: string;
        };
        Snapshot: {
            commit: string;
            configHash: string;
            dependencyFingerprint?: string;
            diagnostics: components["schemas"]["Diagnostic"][];
            edges: components["schemas"]["Dependency"][];
            engine: string;
            files: {
                [key: string]: components["schemas"]["FileRecord"];
            };
            graphHash: string;
            id: string;
            modules: {
                [key: string]: components["schemas"]["Module"];
            };
            profile: components["schemas"]["Profile"];
            repository: string;
            /** Format: int32 */
            schemaVersion: number;
            sourceHash: string;
            topologyHash: string;
            tree: string;
        };
        SourceDiff: {
            after?: null | components["schemas"]["SourceResponse"];
            afterName?: string | null;
            before?: null | components["schemas"]["SourceResponse"];
            beforeName?: string | null;
            comparison: string;
            flags: string[];
            unified: string;
        };
        SourceRef: {
            blob: string;
            endByte?: number | null;
            endLine: number;
            path: string;
            startByte?: number | null;
            startLine: number;
        };
        SourceResponse: {
            anchor?: null | components["schemas"]["SourceRef"];
            blob: string;
            language: string;
            path: string;
            snapshot: string;
            text: string;
        };
        TraverseRequest: {
            depth?: number | null;
            direction?: string | null;
            edgeTypes?: string[];
            limit?: number | null;
            snapshot: string;
            start: string;
        };
        ViewEdge: {
            added: number;
            count: number;
            from: string;
            id: string;
            kind: string;
            relationships: components["schemas"]["EdgeChange"][];
            removed: number;
            to: string;
            unchanged: number;
        };
        ViewNode: components["schemas"]["Module"] & {
            before?: string | null;
            boundary: boolean;
            changedDescendants: number;
            childCount: number;
            descendantCount: number;
            flags: string[];
            movedOut: number;
            side: string;
            snapshot: string;
        };
    };
    responses: never;
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
}
export type $defs = Record<string, never>;
export interface operations {
    commits: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    createComparison: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CompareRequest"];
            };
        };
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ReadyOrJob"];
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    comparison: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Comparison"];
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    changes: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    sourceDiff: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SourceDiff"];
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    graphView: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["GraphRequest"];
            };
        };
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["GraphView"];
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    edge: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
                edge: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    job: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Job"];
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    cancel: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Job"];
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    events: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "text/event-stream": string;
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    path: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PathRequest"];
            };
        };
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    refs: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    createSnapshot: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["BuildRequest"];
            };
        };
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ReadyOrJob"];
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    snapshot: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    children: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    diagnostics: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    modulesAt: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    module: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
                module: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    receipt: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["BuildReceipt"];
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    search: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    source: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path: {
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SourceResponse"];
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    traversal: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["TraverseRequest"];
            };
        };
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
    workspace: {
        parameters: {
            query?: {
                revision?: string;
                parent?: string;
                q?: string;
                kind?: string;
                language?: string;
                path?: string;
                module?: string;
                status?: string;
                offset?: number;
                limit?: number;
                line?: number;
                byte?: number;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Success */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        [key: string]: unknown;
                    };
                };
            };
            /** @description Invalid request or unavailable artifact */
            400: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
        };
    };
}
