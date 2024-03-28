mkdir -p ./artifacts/motherspace/0.1.0
cp ./target/ink/motherspace/motherspace.contract ./artifacts/motherspace/0.1.0
cp ./target/ink/motherspace/motherspace.json ./artifacts/motherspace/0.1.0
cp ./target/ink/motherspace/motherspace.wasm ./artifacts/motherspace/0.1.0

mkdir -p ./artifacts/space/0.1.0
cp ./target/ink/space/space.contract ./artifacts/space/0.1.0
cp ./target/ink/space/space.json ./artifacts/space/0.1.0
cp ./target/ink/space/space.wasm ./artifacts/space/0.1.0

mkdir -p ./artifacts/plugins/posts/0.2.0

cp ./target/ink/posts/posts.contract ./artifacts/plugins/posts/0.2.0
cp ./target/ink/posts/posts.json ./artifacts/plugins/posts/0.2.0
cp ./target/ink/posts/posts.wasm ./artifacts/plugins/posts/0.2.0

mkdir -p ./artifacts/plugins/posts_launcher/0.1.0

cp ./target/ink/posts_launcher/posts_launcher.contract ./artifacts/plugins/posts_launcher/0.1.0
cp ./target/ink/posts_launcher/posts_launcher.json ./artifacts/plugins/posts_launcher/0.1.0
cp ./target/ink/posts_launcher/posts_launcher.wasm ./artifacts/plugins/posts_launcher/0.1.0


mkdir -p ./artifacts/plugins/flipper/0.1.0

cp ./target/ink/flipper/flipper.contract ./artifacts/plugins/flipper/0.1.0
cp ./target/ink/flipper/flipper.json ./artifacts/plugins/flipper/0.1.0
cp ./target/ink/flipper/flipper.wasm ./artifacts/plugins/flipper/0.1.0

mkdir -p ./artifacts/plugins/flipper_launcher/0.1.0

cp ./target/ink/flipper_launcher/flipper_launcher.contract ./artifacts/plugins/flipper_launcher/0.1.0
cp ./target/ink/flipper_launcher/flipper_launcher.json ./artifacts/plugins/flipper_launcher/0.1.0
cp ./target/ink/flipper_launcher/flipper_launcher.wasm ./artifacts/plugins/flipper_launcher/0.1.0


mkdir -p ./artifacts/plugins/polls/0.1.0

cp ./target/ink/polls/polls.contract ./artifacts/plugins/polls/0.1.0
cp ./target/ink/polls/polls.json ./artifacts/plugins/polls/0.1.0
cp ./target/ink/polls/polls.wasm ./artifacts/plugins/polls/0.1.0

mkdir -p ./artifacts/plugins/polls_launcher/0.1.0

cp ./target/ink/polls_launcher/polls_launcher.contract ./artifacts/plugins/polls_launcher/0.1.0
cp ./target/ink/polls_launcher/polls_launcher.json ./artifacts/plugins/polls_launcher/0.1.0
cp ./target/ink/polls_launcher/polls_launcher.wasm ./artifacts/plugins/polls_launcher/0.1.0

echo DONE!