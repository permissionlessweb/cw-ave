#!/bin/bash

# Run schema command for cw-ave and move the schema file
echo "Updating schema for cw-ave..."
cargo schema --package cw-ave
mv schema/cw-ave.json ./contracts/cw-ave/schema/

# Run schema command for cw-ave-factory and move the schema file
echo "Updating schema for cw-ave-factory..."
cargo schema --package cw-ave-factory
mv schema/cw-ave-factory.json ./contracts/cw-ave-factory/schema/

echo "Schema update completed."

## generate ts code
cd ts && yarn codegen