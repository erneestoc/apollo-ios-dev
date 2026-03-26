#!/usr/bin/env python3
"""Generate a large GraphQL schema and operations for benchmarking.

Produces a deterministic (seeded) schema with:
- 500+ types (objects, interfaces, unions, enums, input objects)
- Deep nesting (5+ levels of object->object references)
- All GraphQL features: arguments, directives, deprecation, custom scalars
- 200+ operations (queries, mutations, fragments)

Usage:
    python3 generate-huge-schema.py <output_dir>

Writes:
    <output_dir>/schema.graphqls
    <output_dir>/operations.graphql
"""

import os
import random
import sys


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <output_dir>", file=sys.stderr)
        sys.exit(1)

    output_dir = sys.argv[1]
    os.makedirs(output_dir, exist_ok=True)

    # Fixed seed for deterministic output
    rng = random.Random(42)

    schema_lines = []
    operations_lines = []

    # =========================================================================
    # Custom scalars
    # =========================================================================
    custom_scalars = ["DateTime", "JSON", "URL", "BigInt", "Decimal", "UUID",
                      "Email", "PhoneNumber", "Latitude", "Longitude"]
    for s in custom_scalars:
        schema_lines.append(f'scalar {s}')
    schema_lines.append("")

    # =========================================================================
    # Directives
    # =========================================================================
    schema_lines.append('directive @cacheControl(maxAge: Int, scope: CacheControlScope) on FIELD_DEFINITION | OBJECT | INTERFACE')
    schema_lines.append('directive @deprecated(reason: String = "No longer supported") on FIELD_DEFINITION | ENUM_VALUE | ARGUMENT_DEFINITION')
    schema_lines.append("")

    # =========================================================================
    # Enums
    # =========================================================================
    enum_names = []
    enum_defs = {}  # name -> list of values
    base_enum_prefixes = [
        "Status", "Priority", "Category", "Role", "Visibility",
        "SortOrder", "FilterType", "Permission", "Tier", "Region",
        "Color", "Size", "Shape", "Direction", "Frequency",
        "Quality", "Mode", "Phase", "Level", "Grade",
    ]
    for i in range(60):
        prefix = base_enum_prefixes[i % len(base_enum_prefixes)]
        name = f"{prefix}Type{i}"
        enum_names.append(name)
        values = []
        num_values = rng.randint(3, 8)
        for j in range(num_values):
            v = f"VALUE_{j}"
            deprecated = ""
            if j == 0 and i % 5 == 0:
                deprecated = ' @deprecated(reason: "Use VALUE_1 instead")'
            values.append(f"  {v}{deprecated}")
        enum_defs[name] = [f"VALUE_{j}" for j in range(num_values)]
        schema_lines.append(f"enum {name} {{")
        schema_lines.extend(values)
        schema_lines.append("}")
        schema_lines.append("")

    schema_lines.append("enum CacheControlScope { PUBLIC PRIVATE }")
    schema_lines.append("")

    # =========================================================================
    # Input objects
    # =========================================================================
    input_names = []
    for i in range(50):
        name = f"FilterInput{i}"
        input_names.append(name)
        schema_lines.append(f"input {name} {{")
        schema_lines.append(f"  id: ID")
        schema_lines.append(f"  name: String")
        schema_lines.append(f"  limit: Int = 10")
        schema_lines.append(f"  offset: Int = 0")
        schema_lines.append(f"  active: Boolean")
        if i > 0:
            # Reference a previous input for nesting
            schema_lines.append(f"  nestedFilter: FilterInput{rng.randint(0, i - 1)}")
        if i < len(enum_names):
            schema_lines.append(f"  sortBy: {enum_names[i]}")
        for j in range(rng.randint(0, 3)):
            scalar = rng.choice(custom_scalars)
            schema_lines.append(f"  custom{j}: {scalar}")
        schema_lines.append("}")
        schema_lines.append("")

    # =========================================================================
    # Interfaces
    # =========================================================================
    interface_names = []
    interface_fields = {}  # name -> list of (field_name, type_str)
    for i in range(40):
        name = f"INode{i}"
        interface_names.append(name)
        fields = [
            ("id", "ID!"),
            ("createdAt", "DateTime!"),
            ("updatedAt", "DateTime"),
        ]
        if i > 0 and i % 3 == 0:
            # Extend another interface
            parent = interface_names[rng.randint(0, i - 1)]
            schema_lines.append(f"interface {name} implements {parent} {{")
            # Include parent fields
            for fn, ft in interface_fields.get(parent, []):
                fields.append((fn, ft))
        else:
            schema_lines.append(f"interface {name} {{")

        # Add unique fields
        fields.append((f"label{i}", "String"))
        fields.append((f"count{i}", "Int"))
        interface_fields[name] = fields

        seen = set()
        for fn, ft in fields:
            if fn not in seen:
                schema_lines.append(f"  {fn}: {ft}")
                seen.add(fn)
        schema_lines.append("}")
        schema_lines.append("")

    # =========================================================================
    # Object types (the bulk - 350+ types with deep nesting)
    # =========================================================================
    object_names = []
    object_field_info = {}  # name -> list of (field_name, type_str, has_args)

    # Level 0: leaf types (no object references)
    for i in range(80):
        name = f"LeafEntity{i}"
        object_names.append(name)
        implements = ""
        if i < len(interface_names):
            implements = f" implements {interface_names[i % len(interface_names)]}"

        schema_lines.append(f"type {name}{implements} {{")
        fields = []
        # Interface fields
        if implements:
            iface = interface_names[i % len(interface_names)]
            for fn, ft in interface_fields.get(iface, []):
                schema_lines.append(f"  {fn}: {ft}")
                fields.append((fn, ft, False))
        else:
            schema_lines.append("  id: ID!")
            fields.append(("id", "ID!", False))

        schema_lines.append(f"  name: String!")
        fields.append(("name", "String!", False))
        schema_lines.append(f"  description: String")
        fields.append(("description", "String", False))
        schema_lines.append(f"  value{i}: Int")
        fields.append((f"value{i}", "Int", False))

        if i < len(enum_names):
            schema_lines.append(f"  status: {enum_names[i]}")
            fields.append(("status", enum_names[i], False))

        for j in range(rng.randint(1, 4)):
            scalar = rng.choice(custom_scalars)
            fname = f"extra{j}"
            schema_lines.append(f"  {fname}: {scalar}")
            fields.append((fname, scalar, False))

        object_field_info[name] = fields
        schema_lines.append("}")
        schema_lines.append("")

    # Level 1: types referencing leaf types
    for i in range(80):
        name = f"MidEntity{i}"
        object_names.append(name)
        implements = ""
        if i % 4 == 0 and i // 4 < len(interface_names):
            implements = f" implements {interface_names[i // 4]}"

        schema_lines.append(f"type {name}{implements} {{")
        fields = []

        if implements:
            iface = interface_names[i // 4]
            for fn, ft in interface_fields.get(iface, []):
                schema_lines.append(f"  {fn}: {ft}")
                fields.append((fn, ft, False))
        else:
            schema_lines.append("  id: ID!")
            fields.append(("id", "ID!", False))

        schema_lines.append(f"  title: String!")
        fields.append(("title", "String!", False))

        # Reference 2-4 leaf types
        for j in range(rng.randint(2, 4)):
            ref = rng.choice(object_names[:80])  # Only leaf types
            fname = f"leaf{j}"
            schema_lines.append(f"  {fname}: {ref}")
            fields.append((fname, ref, False))

        # List field
        ref = rng.choice(object_names[:80])
        schema_lines.append(f"  items: [{ref}!]!")
        fields.append(("items", f"[{ref}!]!", False))

        # Field with arguments
        schema_lines.append(f"  search(query: String!, limit: Int = 10): [{rng.choice(object_names[:80])}]")
        fields.append(("search", f"[{rng.choice(object_names[:80])}]", True))

        # Deprecated field
        if i % 3 == 0:
            schema_lines.append(f'  oldField: String @deprecated(reason: "Use title instead")')
            fields.append(("oldField", "String", False))

        object_field_info[name] = fields
        schema_lines.append("}")
        schema_lines.append("")

    # Level 2: types referencing mid types
    for i in range(70):
        name = f"HighEntity{i}"
        object_names.append(name)
        schema_lines.append(f"type {name} {{")
        fields = []
        schema_lines.append("  id: ID!")
        fields.append(("id", "ID!", False))
        schema_lines.append(f"  name: String!")
        fields.append(("name", "String!", False))

        for j in range(rng.randint(2, 5)):
            ref_idx = 80 + rng.randint(0, 79)  # MidEntity range
            ref = object_names[ref_idx]
            fname = f"mid{j}"
            schema_lines.append(f"  {fname}: {ref}")
            fields.append((fname, ref, False))

        # Connection-style field
        ref_idx = 80 + rng.randint(0, 79)
        schema_lines.append(f"  connection(first: Int, after: String): [{object_names[ref_idx]}!]!")
        fields.append(("connection", f"[{object_names[ref_idx]}!]!", True))

        if i < len(input_names):
            schema_lines.append(f"  filtered(filter: {input_names[i]}): [{rng.choice(object_names[:160])}]")
            fields.append(("filtered", f"[{rng.choice(object_names[:160])}]", True))

        object_field_info[name] = fields
        schema_lines.append("}")
        schema_lines.append("")

    # Level 3: types referencing high types (deep nesting)
    for i in range(50):
        name = f"TopEntity{i}"
        object_names.append(name)
        schema_lines.append(f"type {name} {{")
        fields = []
        schema_lines.append("  id: ID!")
        fields.append(("id", "ID!", False))
        schema_lines.append(f"  label: String!")
        fields.append(("label", "String!", False))

        for j in range(rng.randint(2, 4)):
            ref_idx = 160 + rng.randint(0, 69)  # HighEntity range
            ref = object_names[ref_idx]
            fname = f"high{j}"
            schema_lines.append(f"  {fname}: {ref}")
            fields.append((fname, ref, False))

        for j in range(rng.randint(1, 3)):
            ref_idx = 80 + rng.randint(0, 79)  # MidEntity range
            ref = object_names[ref_idx]
            fname = f"detail{j}"
            schema_lines.append(f"  {fname}: {ref}")
            fields.append((fname, ref, False))

        object_field_info[name] = fields
        schema_lines.append("}")
        schema_lines.append("")

    # Level 4: root-level aggregate types (deepest nesting)
    for i in range(30):
        name = f"RootAggregate{i}"
        object_names.append(name)
        schema_lines.append(f"type {name} {{")
        fields = []
        schema_lines.append("  id: ID!")
        fields.append(("id", "ID!", False))

        for j in range(rng.randint(2, 4)):
            ref_idx = 230 + rng.randint(0, 49)  # TopEntity range
            ref = object_names[ref_idx]
            fname = f"top{j}"
            schema_lines.append(f"  {fname}: {ref}")
            fields.append((fname, ref, False))

        for j in range(rng.randint(1, 3)):
            ref_idx = 160 + rng.randint(0, 69)  # HighEntity range
            ref = object_names[ref_idx]
            fname = f"aggregate{j}"
            schema_lines.append(f"  {fname}: {ref}")
            fields.append((fname, ref, False))

        schema_lines.append(f"  metadata: JSON")
        fields.append(("metadata", "JSON", False))

        object_field_info[name] = fields
        schema_lines.append("}")
        schema_lines.append("")

    # =========================================================================
    # Unions
    # =========================================================================
    union_names = []
    for i in range(30):
        name = f"SearchResult{i}"
        union_names.append(name)
        # Pick 3-6 random object types for each union
        members = rng.sample(object_names[:230], min(rng.randint(3, 6), len(object_names[:230])))
        schema_lines.append(f"union {name} = {' | '.join(members)}")
        schema_lines.append("")

    # =========================================================================
    # Query, Mutation, Subscription root types
    # =========================================================================
    schema_lines.append("type Query {")
    # Query fields referencing various types
    for i in range(60):
        obj = object_names[rng.randint(0, len(object_names) - 1)]
        schema_lines.append(f"  getEntity{i}(id: ID!): {obj}")
    for i in range(30):
        obj = object_names[rng.randint(0, len(object_names) - 1)]
        schema_lines.append(f"  listEntities{i}(limit: Int = 20, offset: Int = 0): [{obj}!]!")
    for i in range(20):
        union = union_names[i % len(union_names)]
        schema_lines.append(f"  search{i}(query: String!, filter: {input_names[i % len(input_names)]}): [{union}!]!")
    schema_lines.append("}")
    schema_lines.append("")

    schema_lines.append("type Mutation {")
    for i in range(40):
        obj = object_names[rng.randint(0, len(object_names) - 1)]
        input_type = input_names[i % len(input_names)]
        schema_lines.append(f"  createEntity{i}(input: {input_type}!): {obj}")
    for i in range(20):
        obj = object_names[rng.randint(0, len(object_names) - 1)]
        schema_lines.append(f"  updateEntity{i}(id: ID!, name: String): {obj}")
    for i in range(10):
        schema_lines.append(f"  deleteEntity{i}(id: ID!): Boolean!")
    schema_lines.append("}")
    schema_lines.append("")

    schema_lines.append("type Subscription {")
    for i in range(10):
        obj = object_names[rng.randint(0, len(object_names) - 1)]
        schema_lines.append(f"  onEntityUpdated{i}(id: ID!): {obj}")
    schema_lines.append("}")
    schema_lines.append("")

    # =========================================================================
    # Operations
    # =========================================================================

    def scalar_subfields(type_str):
        """Return simple scalar fields for a selection set."""
        return ["id", "name", "__typename"]

    def make_selection(obj_name, depth=0, max_depth=3):
        """Build a selection set string for an object type."""
        lines = []
        if obj_name not in object_field_info:
            return ["id", "__typename"]

        fields = object_field_info[obj_name]
        for fn, ft, has_args in fields:
            # Skip fields with required arguments (complex to generate)
            if has_args:
                continue
            # Check if type references another object
            base_type = ft.replace("[", "").replace("]", "").replace("!", "")
            if base_type in object_field_info and depth < max_depth:
                sub = make_selection(base_type, depth + 1, max_depth)
                lines.append(f"{fn} {{")
                for sl in sub:
                    lines.append(f"  {sl}")
                lines.append("}")
            else:
                lines.append(fn)
        return lines if lines else ["id", "__typename"]

    # Queries (100)
    for i in range(100):
        query_name = f"GetEntity{i}Query"
        obj_idx = rng.randint(0, len(object_names) - 1)
        obj = object_names[obj_idx]
        # Limit depth to keep operations reasonable
        depth = rng.randint(1, 3)
        sel = make_selection(obj, 0, depth)
        operations_lines.append(f"query {query_name}($id: ID!) {{")
        operations_lines.append(f"  getEntity{i % 60}(id: $id) {{")
        for s in sel:
            operations_lines.append(f"    {s}")
        operations_lines.append("  }")
        operations_lines.append("}")
        operations_lines.append("")

    # List queries (40)
    for i in range(40):
        query_name = f"ListEntities{i}Query"
        obj_idx = rng.randint(0, len(object_names) - 1)
        obj = object_names[obj_idx]
        depth = rng.randint(1, 2)
        sel = make_selection(obj, 0, depth)
        operations_lines.append(f"query {query_name}($limit: Int, $offset: Int) {{")
        operations_lines.append(f"  listEntities{i % 30}(limit: $limit, offset: $offset) {{")
        for s in sel:
            operations_lines.append(f"    {s}")
        operations_lines.append("  }")
        operations_lines.append("}")
        operations_lines.append("")

    # Mutations (40)
    for i in range(40):
        mutation_name = f"CreateEntity{i}Mutation"
        input_type = input_names[i % len(input_names)]
        obj_idx = rng.randint(0, len(object_names) - 1)
        obj = object_names[obj_idx]
        sel = make_selection(obj, 0, 1)
        operations_lines.append(f"mutation {mutation_name}($input: {input_type}!) {{")
        operations_lines.append(f"  createEntity{i % 40}(input: $input) {{")
        for s in sel:
            operations_lines.append(f"    {s}")
        operations_lines.append("  }")
        operations_lines.append("}")
        operations_lines.append("")

    # Fragments (40)
    used_fragment_names = set()
    for i in range(40):
        obj_idx = rng.randint(0, min(len(object_names) - 1, 229))
        obj = object_names[obj_idx]
        frag_name = f"{obj}Fragment{i}"
        if frag_name in used_fragment_names:
            frag_name = f"{obj}Fragment{i}v2"
        used_fragment_names.add(frag_name)
        sel = make_selection(obj, 0, 2)
        operations_lines.append(f"fragment {frag_name} on {obj} {{")
        for s in sel:
            operations_lines.append(f"  {s}")
        operations_lines.append("}")
        operations_lines.append("")

    # Queries using fragments (20)
    fragment_list = list(used_fragment_names)
    for i in range(20):
        query_name = f"GetWithFragment{i}Query"
        frag = fragment_list[i % len(fragment_list)]
        # Find the type this fragment is on (parse from name)
        operations_lines.append(f"query {query_name}($id: ID!) {{")
        operations_lines.append(f"  getEntity{i % 60}(id: $id) {{")
        operations_lines.append(f"    ...{frag}")
        operations_lines.append("  }")
        operations_lines.append("}")
        operations_lines.append("")

    # =========================================================================
    # Write output files
    # =========================================================================
    schema_path = os.path.join(output_dir, "schema.graphqls")
    with open(schema_path, "w") as f:
        f.write("\n".join(schema_lines))
    print(f"  Generated schema: {schema_path} ({len(schema_lines)} lines)")

    operations_path = os.path.join(output_dir, "operations.graphql")
    with open(operations_path, "w") as f:
        f.write("\n".join(operations_lines))
    print(f"  Generated operations: {operations_path} ({len(operations_lines)} lines)")

    # Print stats
    total_types = (len(enum_names) + 1 +  # enums + CacheControlScope
                   len(input_names) +
                   len(interface_names) +
                   len(object_names) +
                   len(union_names) +
                   3 +  # Query, Mutation, Subscription
                   len(custom_scalars))
    print(f"  Total types: {total_types}")
    print(f"    Scalars: {len(custom_scalars)}")
    print(f"    Enums: {len(enum_names) + 1}")
    print(f"    Input objects: {len(input_names)}")
    print(f"    Interfaces: {len(interface_names)}")
    print(f"    Objects: {len(object_names) + 3}")
    print(f"    Unions: {len(union_names)}")
    print(f"  Total operations: {100 + 40 + 40 + 40 + 20}")


if __name__ == "__main__":
    main()
