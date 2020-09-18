function move_made(buttObj) {
    var move = buttObj.id;
    var curr_game_name = buttObj.name;
    var data = {"move": move, "curr_game_name": curr_game_name};
    as_json = JSON.stringify(data);
    console.log(as_json);

    // POST
    fetch('/passData', {

        // Specify the method
        method: 'POST',

        // A JSON payload
        body: as_json,
        cache: "no-cache",
        headers: new Headers({
          "content-type": "application/json"
        })
    }).then(function (response) { // At this point, Flask has printed our JSON
        return response.json();
    }).then(function (json) {

        console.log('POST response: ');
        console.log(JSON.stringify(json));

        // Loop through all positions on the board and set the colours to blank
        // Enable all buttons
        
        var all_ids = ['a1', 'a2', 'a3', 'a4', 'a5', 'b1', 'b2', 'b3', 'b4', 'b5', 'c1', 'c2', 'c3', 'c4', 'c5', 'd1', 'd2', 'd3', 'd4', 'd5', 'e1', 'e2', 'e3', 'e4', 'e5'];

        for (i of all_ids){
            document.getElementById(i).style.backgroundColor = "white";
            document.getElementById(i).disabled = false;
        }

        var all_data = JSON.parse(JSON.stringify(json));
        // Loop through player 1 positions, set colours and disable
        p1_pos = all_data['p1'];
        for (pos of p1_pos){
            document.getElementById(pos).style.backgroundColor = "chartreuse";
            document.getElementById(pos).disabled = true;
        }

        // Loop through player 2 positions, set colours and disable
        p2_pos = all_data['p2'];
        for (pos of p2_pos){
            document.getElementById(pos).style.backgroundColor = "tomato";
            document.getElementById(pos).disabled = true;
        }

        // Check if game has finished
        var game_over = all_data['game_over']
        if (game_over == 1) {
            for (pos of all_ids){
                document.getElementById(pos).disabled = true;
            }
            window.alert("You just won!! Well Done!")
        }


    });

    // Get all player 1 positions
    // Get all player 2 positions

    // fetch('/passData')
    //     .then(function (response) {
    //         return response.json();
    //     })
    //     .then(function (json) {
    //         p1_pos = json["p1"];
    //         p2_pos = json["p2"];
    //         console.log(p1_pos);
    //     });


    // Make correct counters appear for each player
};