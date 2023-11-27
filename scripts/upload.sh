cd contracts/space
cargo contract upload --suri //Alice -x ../../target/ink/space/space.contract

cd ../motherspace
cargo contract upload --suri //Alice -x ../../target/ink/motherspace/motherspace.contract