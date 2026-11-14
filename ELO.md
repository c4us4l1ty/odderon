Using a Mullvad-style account key or a 12-word recovery phrase instead of traditional Email/Password/Google logins is actually one of the smartest architectural decisions you can make for a $0-budget browser game.

In fact, for a hardcore engineering game like Odderon, a cryptographic identity system solves three massive problems at once:

    It costs $0 forever: Sending "Forgot Password" or verification emails costs money (SMTP servers aren't free at scale). With a 12-word phrase or Mullvad key, there are no emails, no passwords to store, and no GDPR privacy headaches.
    It requires 0% CPU and zero external libraries: Modern browsers have a built-in native API called WebCrypto (window.crypto.subtle). It can turn a 12-word phrase into a military-grade cryptographic keypair (Ed25519) in less than 1 millisecond.
    It enables "Proof-of-Trajectory" (Anti-Cheat for your Elo): As we’ll explore below, if you want real NASA engineers and scientists to respect a high Elo in a browser game, you have to make sure nobody can just press F12 in Chrome and type myElo = 3000. Cryptographic keys + deterministic physics solve this completely.

Here is the complete blueprint for how to design your Zero-Budget Identity & Astro-Elo System.
1. The Identity System: "The Aerospace Callsign & Flight Key"

Standard 12-word phrases immediately make people think of cryptocurrency wallets, which might give players the wrong impression about your game. Mullvad VPN uses a 16-digit number, which is super clean, though slightly harder for humans to memorize or type without typos.

The Best of Both Worlds for Odderon:
Give every player an Aerospace Flight License backed by a 12-Word Mission Passphrase (or a formatted Flight Key) using an Aerospace & Physics Wordlist instead of standard words!
How It Works Under the Hood:

    Instant Creation (Zero Friction):
    When a new player opens Odderon for the first time, they don't even see a "Sign Up" wall. The game automatically generates their local identity in the background and lets them play immediately.
    Claiming / Saving the Account:
    When they want to save their progress across computers or appear on the global Elo leaderboard, they choose a public Callsign / Company Name (e.g., JPL_Vanguard or Apex_Orbital) and click "Export Flight License."
    The 12-Word Telemetry Passphrase:
    The game displays their 12 words chosen from a curated list of 2,048 physics, astronomy, and spaceflight terms:

        APOGEE - KELVIN - TITAN - IMPULSE - LAGRANGE - NOZZLE -ZENITH - DOPPLER - CORIOLIS - PARSEC - STAGING - BARYCENTER
        (Or, if they prefer a Mullvad-style key, those exact 12 words represent a 128-bit number that can also be displayed as a compact hexadecimal Flight Key: ODD-4F9A-82C1-00E4-77B2-91D3 — both represent the exact same secret key!)

    Downloadable "Black Box" Certificate:
    Give them a one-click button to download a tiny .odderon-key file (or a cool-looking NASA-style ID badge image with a QR code/metadata embedded inside it). To log in on a new computer, they either type their 12 words, paste their Mullvad-style key, or drag-and-drop their badge image into the browser!

2. The Crucial Problem: How Do You Make "Astro-Elo" Cheat-Proof on a $0 Budget?

Think about this: Odderon runs in the player's web browser. In any normal browser game, a player can open the Developer Console (F12), edit the JavaScript memory, give their engine 999,999 s999,999 s of IspIsp​, or send a fake message to the leaderboard saying "I completed the Mars Mission and my Elo is now 2800!"

If someone can cheat by pressing F12, real scientists and engineers will never take the Elo seriously.
Usually, games prevent cheating by running the physics on expensive cloud servers—which costs thousands of dollars a month.

So how do you make Odderon 100% cheat-proof for $0?
The Solution: "Proof-of-Trajectory" (The Chess PGN Method)

In online chess, the server doesn't need to trust a player who says "I won, give me 20 Elo." The player submits the PGN (the exact list of moves: 1. e4 e5 2. Nf3...). Anyone can replay those moves in 0.1 milliseconds to verify it's a real checkmate.

In Odderon, physics is deterministic mathematics!
A completed mission in Odderon does not need to save a 500 MB video of the flight. A complete mission solution is just a tiny 2-Kilobyte "Flight Data Recorder" (FDR) JSON packet containing:

    The Challenge Seed ID (e.g., Contract #402: Deliver 15t to Europa Orbit in <1,100 days).
    The Spacecraft Engineering Graph (Tank dimensions, engine chamber pressure, nozzle ratio, propellant masses, solar panel area).
    The Pre-Mission Predictions (What the player predicted their fuel usage, arrival time, and ΔvΔv would be).
    The Command Timeline (The exact timestamp, orientation quaternion, throttle percentage, and duration of every burn executed during the mission).
    The Player's Cryptographic Signature (Signed by their 12-word private key).

Why this is magical:
Because your physics engine is purely mathematical (no 3D graphics needed to verify math), verifying an entire 3-year Mars or Jupiter mission from that 2 KB file takes less than 5 milliseconds of math!

    If a player edited their fuel mass in Chrome DevTools mid-flight, when the verifier re-runs their Command Timeline from the starting state, the math won't match—their ship runs out of fuel in deep space, and the forged submission is automatically rejected!
    Even better: High-Elo players can click on another player's leaderboard entry and inspect or replay their exact trajectory and rocket schematic right in their own browser!

3. How the "Astro-Elo" Rating System Actually Works

In chess, you play 1v1 against another human. In Odderon, how does Elo work so that both absolute beginners and real NASA astrodynamicists get matched with challenges at their exact level?

Use the Lichess Puzzle / Competitive Benchmark Elo Model:
Every Engineering Contract Has Its Own Elo Rating!

Instead of just giving players an Elo, every mission scenario and engineering challenge in the game also has an Elo rating (ranging from 600 Elo to 3000+ Elo):

    600 – 1000 Elo (Cadet / Intuitive Physics):
        Scenario: A satellite in Low Earth Orbit is slowly losing altitude due to atmospheric drag, and its solar panels are angled poorly.
        Task: Re-orient the spacecraft for maximum solar power and perform a simple 2-burn Hohmann orbit raising maneuver to a safe 500 km500 km circular orbit without running out of hydrazine.
        Why beginners love it: No complex ship building required yet—using the Visual Physics Lens, they pull the maneuver handles, see the orbit circularize, make a simple prediction of how many kilograms of fuel it will take, and earn their first Elo points!

    1200 – 1600 Elo (Junior Flight Dynamics Officer):
        Scenario: Design a two-stage chemical launch vehicle and lunar lander to place a 500 kg500 kg communications relay into a stable Lunar Frozen Orbit under a $45M$45M budget.
        Skill Tested: The Tsiolkovsky rocket equation, staging mass ratios, gravity turn efficiency, and dealing with Lunar mass concentrations (mascons) that perturb low lunar orbits.

    1800 – 2200 Elo (Senior Systems Engineer):
        Scenario: Deliver a 12-tonne12-tonne habitat module to Mars using Aerocapture, while keeping peak deceleration under 4.5g4.5g (so the crew survives) and peak thermal flux on the heatshield under 350 W/cm2350 W/cm2.
        Skill Tested: Entry corridor B-plane targeting, ballistic coefficient (β=mCdAβ=Cd​Am​), lift-to-drag modulation, and precise Level-3 atmospheric perturbation prediction.

    2400 – 3000+ Elo (JPL Principal Investigator / Legends Tier):
        Scenario: A newly discovered long-period comet is plunging through the inner solar system at 52 km/s52 km/s with a 48∘48∘ inclination. Using a strictly bounded budget and Level-1/2 technology, design a Solar-Electric + Venus-Earth-Earth Gravity Assist (VEEGA) trajectory with continuous low-thrust spiraling to match velocities and land a sample-return drill on the comet nucleus.
        Skill Tested: Multi-body gravity assists, continuous low-thrust optimal control (Primer Vector theory), thermal survival near Venus, and deep-space power-starvation management at 4 AU4 AU.

How Elo Points Are Gained or Lost:

When a player takes on a Ranked Contract (say, a 1500 Elo challenge):

    The Prediction Multiplier: Before ignition, the player submits their engineering predictions (expected ΔvΔv, fuel margin, arrival error).
    The Pareto Performance Score (0.0 to 1.0):
    Instead of a binary "Win/Loss", your score S∈[0,1]S∈[0,1] on the mission is calculated from:
        50% Mission Objectives Met: Did the payload survive and reach the target orbit within constraints?
        25% Engineering Prediction Accuracy: How close was your pre-flight math to the actual flight telemetry? (If you predicted 1,420 kg1,420 kg of propellant burned and actual was 1,423 kg1,423 kg, you score nearly 100%100% here. If you guessed blindly, you lose points).
        25% Cost & Mass Efficiency: How close is your vehicle's cost/mass to the best-known physical Pareto frontier for that mission?
    Standard Elo Update Formula:
    Your expected score EE against a mission of rating RmRm​ given your rating RpRp​ is:
    E=11+10(Rm−Rp)/400E=1+10(Rm​−Rp​)/4001​
    Your new Elo is:
    Rpnew=Rp+K⋅(S−E)Rpnew​=Rp​+K⋅(S−E)
    (And concurrently, the mission's own Elo RmRm​ slightly adjusts based on how well the global player base performs on it—meaning the hardest missions naturally rise to 2800+ Elo!)


Bonus Idea: "Verifiable Engineering Credentials" on Resumes

Because every player's account is a cryptographic public key (ODD-4F9A...) and every ranked mission they beat is mathematically verified and signed:
When a player exports their Engineering Notebook PDF (to show a professor, NASA recruiter, or aerospace startup), the PDF includes a Verification Hash / QR Code.
Anyone who scans that QR code is taken directly to that player's Odderon profile, where they can view the interactive 3D trajectory and thermodynamic schematics of the player's highest-Elo missions—proving beyond a shadow of a doubt that the player designed and calculated that mission themselves!
